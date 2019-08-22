# Discord Mods Bot
A discord bot written in rust.  

# Commands
Commands for the bot are managed using the `Commands` struct.  The `.add` method
is used to define commands for the bot to react to.  

## Defining Commands
Commands are defined using a string and a function.  The string is the command
that the user must type and the function is the handler for that command.  

An example command using an anonymous function as a handler.  
```rust
let mut cmds = Commands::new();

cmds.add("?greet {name}", |args: Args<'_>| -> Result {
  println!("Hello {}!", args.params.get("name").unwrap());
});
```

The same command using a function pointer as a handler.  
```rust
fn print_hello_name<'m>(args: Args<'m>) -> Result {
  println!("Hello {}!", args.params.get("name").unwrap());
};

let mut cmds = Commands::new();
cmds.add("?greet {name}", print_hello_name);
```

## Command Syntax
Commands use a syntax with 3 different kinds of elements that can be used
together.  

+ Static elements must be matched exactly, these are strings like `?talk` and
  `!ban`.  
+ Dynamic elements match any input other than a space.  To use a dynamic
  element in your command use `{key}` where `key` can be any name that
  represents that particular input element.
+ Quoted elements match any input but must be surrounded by quotes.  To use a
  quoted element in your command use `[key]` where `key` can be any name that
  represents that particular input element.  

## Command Handlers
Functions are used as handlers for commands.  Specifically handler functions
must use the following signature: `(Args<'_>) -> Result`.

## Args
The `Args` type encapsulated the parameters extracted from the input as well as
the `Message` and `Context` types from the Serenity crate.  

### Serenity
The library the bot uses to communicate with discord is called Serenity.
Serenity abstracts all communication with discord into methods available through
the `Message` and `Context` types.  

# Features

## Tags
Tags are a simple key value store.  


Lookup a tag
```
?tag {key}
```
Create a tag
```
?tag create {key} [value]
```
Delete a tag
```
?tag delete {key}
```
Get all tags
```
?tags
```
### Ban
Ban a user
```
?ban {user}

```
### Kick
Kick a user
```
?kick {user}
```
### Slowmode
Set slowmode for a channel.  0 seconds disables slowmode.  
```
?slowmode {channel} {seconds}
```

### Code of conduct welcome message
Sets up the code of conduct message with reaction in the specified channel.
Used for assigning talk roles.  
```
?CoC {channel}
```


