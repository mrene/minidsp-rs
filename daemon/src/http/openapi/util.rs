//! Utils verbatim from <https://github.com/GREsau/okapi>

#![allow(dead_code)]
use anyhow::{anyhow, Result};
use okapi::{openapi3::*, Map};
use serde_json::Value;

// FIXME this whole file is a huge mess...

/// Takes a `Responses` struct, and sets the status code to the status code provided for each
/// response in the `Responses`.
pub fn set_status_code(responses: &mut Responses, status: u16) -> Result<()> {
    let old_responses = std::mem::take(&mut responses.responses);
    let new_response = ensure_not_ref(ensure_status_code_exists(responses, status))?;
    for (_, mut response) in old_responses {
        *new_response =
            produce_either_response(new_response.clone(), ensure_not_ref(&mut response)?.clone());
    }
    Ok(())
}

/// Checks if the provided `status` code is in the `responses.responses` field. If it isn't, inserts
/// it.
pub fn ensure_status_code_exists(responses: &mut Responses, status: u16) -> &mut RefOr<Response> {
    responses
        .responses
        .entry(status.to_string())
        .or_insert_with(|| Response::default().into())
}

/// Adds a `Response` to a `Responses` object with the given status code, Content-Type and `MediaType`.
pub fn add_content_response(
    responses: &mut Responses,
    status: u16,
    content_type: impl ToString,
    media: MediaType,
) -> Result<()> {
    let response = ensure_not_ref(ensure_status_code_exists(responses, status))?;
    add_media_type(&mut response.content, content_type, media);
    Ok(())
}

/// Adds the `media` to the given map. If the map already contains a `MediaType` with the given
/// Content-Type, then it will be combined with `media`.
pub fn add_media_type(
    content: &mut Map<String, MediaType>,
    content_type: impl ToString,
    media: MediaType,
) {
    // FIXME these clones shouldn't be necessary
    content
        .entry(content_type.to_string())
        .and_modify(|mt| *mt = accept_either_media_type(mt.clone(), media.clone()))
        .or_insert(media);
}

/// Replaces the Content-Type for all responses with `content_type`.
pub fn set_content_type(responses: &mut Responses, content_type: impl ToString) -> Result<()> {
    for ref mut resp_refor in responses.responses.values_mut() {
        let response = ensure_not_ref(resp_refor)?;
        let content = &mut response.content;
        let mt = if content.values().len() == 1 {
            content.values().next().unwrap().clone()
        } else {
            content.values().fold(Default::default(), |mt, mt2| {
                accept_either_media_type(mt, mt2.clone())
            })
        };
        content.clear();
        content.insert(content_type.to_string(), mt);
    }
    Ok(())
}

/// Adds a `Response` to a `Responses` object with the given status code, Content-Type and `SchemaObject`.
pub fn add_schema_response(
    responses: &mut Responses,
    status: u16,
    content_type: impl ToString,
    schema: SchemaObject,
    example: Option<Value>,
) -> Result<()> {
    let media = MediaType {
        schema: Some(schema),
        example,
        ..Default::default()
    };
    add_content_response(responses, status, content_type, media)
}

/// Merges the the two given `Responses`.
pub fn produce_any_responses(r1: Responses, r2: Responses) -> Result<Responses> {
    let mut result = Responses {
        default: r1.default.or(r2.default),
        responses: r1.responses,
        extensions: extend(r1.extensions, r2.extensions),
    };
    for (status, mut response2) in r2.responses {
        let response1 = ensure_not_ref(
            result
                .responses
                .entry(status)
                .or_insert_with(|| Response::default().into()),
        )?;
        *response1 =
            produce_either_response(ensure_not_ref(&mut response2)?.clone(), response1.clone());
    }
    Ok(result)
}

pub fn ensure_not_ref(response: &mut RefOr<Response>) -> Result<&mut Response> {
    match response {
        RefOr::Ref(_) => Err(anyhow!("Altering Ref responses is not supported.")),
        RefOr::Object(o) => Ok(o),
    }
}

pub fn extend<A, E: Extend<A>>(mut a: E, b: impl IntoIterator<Item = A>) -> E {
    a.extend(b);
    a
}

pub fn produce_either_response(r1: Response, r2: Response) -> Response {
    let description = if r1.description.is_empty() {
        r2.description
    } else if r2.description.is_empty() {
        r1.description
    } else {
        format!("{}\n{}", r1.description, r2.description)
    };

    let mut content = r1.content;
    for (content_type, media) in r2.content {
        add_media_type(&mut content, content_type, media);
    }

    Response {
        description,
        content,
        headers: extend(r1.headers, r2.headers),
        links: extend(r1.links, r2.links),
        extensions: extend(r1.extensions, r2.extensions),
    }
}

pub fn accept_either_media_type(mt1: MediaType, mt2: MediaType) -> MediaType {
    MediaType {
        schema: accept_either_schema(mt1.schema, mt2.schema),
        example: mt1.example.or(mt2.example),
        examples: match (mt1.examples, mt2.examples) {
            (Some(e1), Some(e2)) => Some(extend(e1, e2)),
            (Some(e), None) | (None, Some(e)) => Some(e),
            (None, None) => None,
        },
        encoding: extend(mt1.encoding, mt2.encoding),
        extensions: extend(mt1.extensions, mt2.extensions),
    }
}

pub fn accept_either_schema(
    s1: Option<SchemaObject>,
    s2: Option<SchemaObject>,
) -> Option<SchemaObject> {
    let (s1, s2) = match (s1, s2) {
        (Some(s1), Some(s2)) => (s1, s2),
        (Some(s), None) | (None, Some(s)) => return Some(s),
        (None, None) => return None,
    };
    let mut schema = SchemaObject::default();
    schema.subschemas().any_of = Some(vec![s1.into(), s2.into()]);
    Some(schema)
}
