# RPC Organization
Objective: Allow parameters to be read and written (mutated). Initially only writes will be supported, but it opens the door to keeping state in the server and allowing clients to read the current configuration. 

GRPC? Just HTTP?
What is prost's wrapper type support?
Probably need *everything* inside wrappers to denote missing fields.
Or we use field masks everywhere. Is that even well supported?

If JSON is picked at least serde will do a good job mapping types.
Would be awkward for bigger values (e.g. FIR filters) - at least it's limited to 4k elements max.
    Maybe these can upload a wav file as a regular http upload instead of going through json

What about CBOR?
    It's a superset of JSON but with binary encoding.
    Can have the same API as JSON and CBOR and distinguish based on content-type.
    Both have serde implementations.

## Transport + Serializer
HTTP+JSON/CBOR

## URL / Component layout?
A _lot_ of parameters are simply mappings to write a single float to the dsp memory.
Most lib functions call roundtrip using WriteFloat and a single variable.
Some components are exceptions to this rule.

Property trees are often used to represent structured configuration, esp. in IO device trees.
Things can be organized in a trie, just like on a filesystem.
Each settings can then be controlled via its object.

Is it possible to represent every component as a tree path? 
Ideally, the lib exposed through lib.rs would be using the same representation, so it can work over http as well as on the device itself.

We can define the property tree as a series of node, where leafs have a specific data type, and can be set by a method on that type.

Can we use enums to represent each path component? This way the tree can be parsed using generated strings, and we can work without string parsing at all when operating from the lib, or from a no-std envirionment.

A handler similar to the CLI can be used to apply the settings directly.

The downside of this approach is the lack of an object-style representation. Ideally, we'd be able to update multiple settings in one shot by setting their values directly, and leaving the rest undefined/null.

Maybe the serde model can be used to stream updates to the given paths.

## Take two!
Instead of engineering another object structure around it, why not have a trait exposing writable fields, with a method generated through a custom derive macro.

The trait can expose a way to get a reflection-like hashmap with a means of obtaining another field.
The fields can either be something settable (deserializing its argument), or another fieldmap object (allowing recursion)

Example:

```rust
trait ControllableObject {
    fn fields(&'a self) -> FieldMap<'a>
}

pub struct FieldMap<'a> {
    contents: HashMap<String, Field<'a>>
}

pub struct Field<'a> {
    
}


```

## Take two^H^H^H 1.5
