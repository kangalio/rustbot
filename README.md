# Rustbot

## Inviting the bot

Some permissions are required:
- Send Messages: base command functionality
- Manage Roles: for `?rustify` command
- Manage Messages: for `?cleanup` command
- Add Reactions: for `?rustify` command feedback
Furthermore, the `applications.commands` OAuth2 scope is required for slash commands.

Here's an invite link to an instance hosted by me on my Raspberry Pi, with the permissions and scopes incorporated:
https://discord.com/oauth2/authorize?client_id=804340127433752646&permissions=268445760&scope=bot%20applications.commands

Adjust the client_id in the URL for your own hosted instances of the bot.

## Hosting the bot

Run the bot using `cargo run --release`.

You will need to provide several environment variables. A convenient way to do this is to copy the
`.env.example` file to `.env` and fill out the values. Then run the bot with the `.env` file applied.

Also set `SQLX_OFFLINE` to `true` if you're running the bot for the first time. Otherwise, SQLx
will try to call into the database to check query correctness, which fails if the database hasn't
been set up yet.

Example command-line for Linux:
`set -a && source .env && set +a && SQLX_OFFLINE=true cargo run --release`

### Docker

This project has a Dockerfile, so you can use Docker to run this bot if you wish.
For that, rename the `.env.example` file into `.env`, fill out the values, and run the commands:

```sh
docker build -t rustbot -f Dockerfile .
docker run -d --rm --name rustbot --env-file .env rustbot
```

Warnings: the Dockerfile may sometimes not work correctly because I don't use it for deployment
myself and also know very little about creating Dockerfiles. If the file is not working for you and
you managed to fix it, I would greatly appreciate a pull request! :)

## Credits

This codebase has its roots in [rust-lang/discord-mods-bot](https://github.com/rust-lang/discord-mods-bot/), the Discord bot running on the official Rust server.
