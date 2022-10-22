//! OpenAPI Specification Generator
//! Most code here was adapter from <https://github.com/GREsau/okapi> so it works outside of rocket with some more code
use std::collections::HashMap;

use hyper::http::Method;
use minidsp::{
    model::{Config, Gate, Input, MasterStatus, Peq, StatusSummary},
    Biquad, DeviceInfo, Gain, Source,
};
use okapi::openapi3::*;
use schemars::{
    gen::{SchemaGenerator, SchemaSettings},
    schema::SchemaObject,
    JsonSchema, Map, MapEntry,
};

use super::FormattedError;

pub mod util;

/// A struct that visits all `rocket::Route`s, and aggregates information about them.
#[derive(Debug, Clone)]
pub struct OpenApiGenerator {
    schema_generator: SchemaGenerator,
    operations: Map<String, HashMap<Method, Operation>>,
}

impl OpenApiGenerator {
    /// Create a new `OpenApiGenerator` from the settings provided.
    pub fn new() -> Self {
        OpenApiGenerator {
            schema_generator: SchemaGenerator::new(SchemaSettings::openapi3()),
            operations: Default::default(),
        }
    }

    /// Add a new `HTTP Method` to the collection of endpoints in the `OpenApiGenerator`.
    pub fn add_operation(&mut self, mut op: OperationInfo) {
        if let Some(op_id) = op.operation.operation_id {
            // TODO do this outside add_operation
            op.operation.operation_id = Some(op_id.trim_start_matches(':').replace("::", "_"));
        }
        match self.operations.entry(op.path) {
            MapEntry::Occupied(mut e) => {
                let map = e.get_mut();
                if map.insert(op.method.clone(), op.operation).is_some() {
                    // This will trow a warning if 2 routes have the same path and method
                    // This is allowed by Rocket when a ranking is given for example: `#[get("/user", rank = 2)]`
                    // See: https://rocket.rs/v0.4/guide/requests/#forwarding
                    println!("Warning: Operation replaced for {}:{}", op.method, e.key());
                }
            }
            MapEntry::Vacant(e) => {
                let mut map = HashMap::new();
                map.insert(op.method, op.operation);
                e.insert(map);
            }
        };
    }

    /// Returns a JSON Schema object for the type `T`.
    pub fn json_schema<T: ?Sized + JsonSchema>(&mut self) -> SchemaObject {
        self.schema_generator.subschema_for::<T>().into()
    }

    /// Generate an `OpenApi` specification for all added operations.
    pub fn into_openapi(self) -> OpenApi {
        let mut schema_generator = self.schema_generator;
        let mut schemas = schema_generator.take_definitions();

        for visitor in schema_generator.visitors_mut() {
            for schema in schemas.values_mut() {
                visitor.visit_schema(schema)
            }
        }

        OpenApi {
            openapi: "3.0.0".to_owned(),
            paths: {
                let mut paths = Map::new();
                for (path, map) in self.operations {
                    for (method, op) in map {
                        let path_item = paths.entry(path.clone()).or_default();
                        set_operation(path_item, method, op);
                    }
                }
                paths
            },
            components: Some(Components {
                schemas: schemas.into_iter().map(|(k, v)| (k, v.into())).collect(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    pub fn json_request_body<T: JsonSchema + serde::Serialize>(
        &mut self,
        example: Option<T>,
    ) -> RequestBody {
        let schema = self.json_schema::<T>();
        let example = serde_json::to_value(example).ok();
        RequestBody {
            content: {
                let mut map = Map::new();
                map.insert(
                    "application/json".to_owned(),
                    MediaType {
                        schema: Some(schema),
                        example,
                        ..Default::default()
                    },
                );
                map
            },
            required: true,
            ..Default::default()
        }
    }

    pub fn json_responses<T: JsonSchema + serde::Serialize, E: JsonSchema>(
        &mut self,
        example: Option<T>,
    ) -> Responses {
        let mut responses = Responses::default();
        let example = serde_json::to_value(example).ok();
        util::add_schema_response(
            &mut responses,
            200,
            "application/json",
            self.json_schema::<T>(),
            example,
        )
        .unwrap();
        util::add_schema_response(
            &mut responses,
            500,
            "application/json",
            self.json_schema::<E>(),
            None,
        )
        .unwrap();
        responses
    }

    pub fn json_err_response<E: JsonSchema>(&mut self) -> Responses {
        let mut responses = Responses::default();
        util::add_content_response(&mut responses, 200, "text/plain", MediaType::default())
            .unwrap();
        util::add_schema_response(
            &mut responses,
            500,
            "application/json",
            self.json_schema::<E>(),
            None,
        )
        .unwrap();
        responses
    }
}

pub struct OperationInfo {
    /// The path of the endpoint
    pub path: String,
    /// The HTTP Method of this endpoint.
    pub method: hyper::http::Method,
    /// Contains information to be showed in the documentation about this endpoint.
    pub operation: okapi::openapi3::Operation,
}

fn set_operation(path_item: &mut PathItem, method: Method, op: Operation) {
    // use hyper::http::Method::*;
    let option = match method {
        Method::GET => &mut path_item.get,
        Method::PUT => &mut path_item.put,
        Method::POST => &mut path_item.post,
        Method::DELETE => &mut path_item.delete,
        Method::OPTIONS => &mut path_item.options,
        Method::HEAD => &mut path_item.head,
        Method::PATCH => &mut path_item.patch,
        Method::TRACE => &mut path_item.trace,
        _ => return,
    };
    assert!(option.is_none());
    option.replace(op);
}

pub fn schema() -> OpenApi {
    let mut gen = OpenApiGenerator::new();

    // GET /devices
    {
        let device = super::Device {
            url: "tcp://1.2.3.4:5333".into(),
            version: Some(DeviceInfo {
                hw_id: 10,
                dsp_version: 100,
                serial: 91234,
                fw_major: 1,
                fw_minor: 53,
            }),
            product_name: Some("2x4 HD".into()),
        };
        let responses =
            gen.json_responses::<Vec<super::Device>, FormattedError>(Some(vec![device]));
        gen.add_operation(OperationInfo {
            path: "/devices".to_string(),
            method: Method::GET,
            operation: Operation {
                summary: Some("List available devices".into()),
                responses,
                request_body: None,
                parameters: Vec::new(),
                ..Default::default()
            },
        });
    }

    // GET /devices/:deviceIndex
    {
        let responses = gen.json_responses::<StatusSummary, FormattedError>(Some(StatusSummary {
            master: MasterStatus {
                preset: Some(0),
                source: Some(Source::Toslink),
                volume: Some(Gain(-5f32)),
                mute: Some(false),
                dirac: Some(false),
            },
            input_levels: [-51f32, -50f32].into(),
            output_levels: [-51f32, -50f32, -127f32, -127f32].into(),
        }));
        let param = Parameter {
            name: "deviceIndex".into(),
            location: "path".into(),
            description: None,
            required: true,
            deprecated: false,
            allow_empty_value: false,
            value: ParameterValue::Schema {
                style: None,
                explode: None,
                allow_reserved: false,
                schema: Default::default(),
                example: None,
                examples: None,
            },
            extensions: Default::default(),
        }
        .into();
        gen.add_operation(OperationInfo {
            path: "/devices/{deviceIndex}".to_string(),
            method: Method::GET,
            operation: Operation {
                summary: Some("Status summary".into()),
                responses,
                request_body: None,
                parameters: vec![param],
                ..Default::default()
            },
        });
    }

    // POST /devices/:deviceIndex/config
    {
        let responses = gen.json_err_response::<FormattedError>();
        let request_body = gen.json_request_body(Some(Config {
            master_status: Some(MasterStatus {
                preset: Some(1),
                ..Default::default()
            }),
            inputs: vec![Input {
                index: Some(0),
                gate: Gate {
                    mute: Some(false),
                    gain: Some(Gain(0.)),
                },
                peq: {
                    let eq = Peq {
                        index: Some(0),
                        coeff: Some(Biquad::default()),
                        bypass: Some(false),
                    };
                    vec![eq]
                },
                ..Default::default()
            }],
            ..Default::default()
        }));
        let param = Parameter {
            name: "deviceIndex".into(),
            location: "path".into(),
            description: None,
            required: true,
            deprecated: false,
            allow_empty_value: false,
            value: ParameterValue::Schema {
                style: None,
                explode: None,
                allow_reserved: false,
                schema: Default::default(),
                example: None,
                examples: None,
            },
            extensions: Default::default(),
        }
        .into();
        gen.add_operation(OperationInfo {
            path: "/devices/{deviceIndex}/config".to_string(),
            method: Method::POST,
            operation: Operation {
                summary: Some("Apply configuration changes".into()),
                responses,
                request_body: Some(request_body.into()),
                parameters: vec![param],
                ..Default::default()
            },
        });
    }

    let mut spec = gen.into_openapi();
    spec.info = Info {
        title: "minidsp-rs".into(),
        description: Some("A control interface to MiniDSPs".into()),
        version: env!("CARGO_PKG_VERSION").into(),
        ..Default::default()
    };
    spec
}
