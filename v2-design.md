# v0.2 design
Objective: Intermediate representation that encodes commands into their packets, and can decode packets into commands in order to track changes applied externally

Available parameters are modelled as a property tree where they are accessed via either Read or Write operations. Each tree item has its associated type which can be validated by the Dialect performing command encoding. 

A giant enum represents the scope of reachable keys, since the scope is fairly limited, and I can't find an equivalent of boost property trees in rust, implementations for parsing and serializing keys as well as accesing their type information can be done manually. A proc macro could eventually define this with field attributes.

### Operation
An operation is a verb (Read, Write) along with a target and a value.

### Result
A result contains the (decoded) data read by the operation, an acknowledgement of success, or the error that caused a problem

### Logic for encoding writes 
the dialect recevies the parsed Target and associated Value (as an Operation)
the dialect looks up (provides?) the type information for the given target
the type is validated with the input, and coerced if required (and possible)
the dialect encodes a command to achieve the given operation

### Logic for decoding writes
the command type is extracted from the command
the target is reconstituted from the (CommandType, Address) combo
the Dialect decodes the encoded value for that target
the Operation is returned


### Multiple commands and state keeping
Operations can yield multiple distinct commands, the dialect (or executor?) is responsible for driving the operation to completion.
When decoding, state is kept to maintain partial information until the whole operation can be reconstructed.

```rust
// 
enum Type {
    Int(isize, isize),
    Decibel(f32),
    Float32(f32, f32),
}

```