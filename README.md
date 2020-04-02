# Discord Mods Bot
A discord bot written in rust.  

# Getting Started

## Setup Postgres
This repo ships with a `Dockerfile` for a postgres image you can use in
`postgres-docker/Dockerfile`.

Build the docker image
```sh
docker build -t postgresql postgres-docker/
```

Run the docker image
```sh
docker run --rm -P --name database postgresql
```

Find out the port the postgres instance is running on
```sh
docker ps
```

## Get the bot running

Build the docker image for the bot
```sh
docker build -t discordbot .
```
A number of environment variables are required to run the bot.  Many of these
environment variables come from discord, this means you will need to have your
own guild setup to test the bot in.  

```sh
MOD_ID is the id of the mod role
TALK_ID is the id of the talk role
DISCORD_TOKEN is the token used to connect to discord
DATABASE_URL is the url where the database is running, you will need to update
the port to the port your instance of postgres is running on
```
Once you have your guild setup, you can run the bot
```sh
docker run -e "MOD_ID=" -e "TALK_ID=" -e "DISCORD_TOKEN=" -e "DATABASE_URL=postgres://docker:docker@172.17.0.1:32768" --add-host=database:172.17.0.2 --rm -it discordbot
```

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


