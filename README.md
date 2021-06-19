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

Run the bot using `cargo run --release`. You will need to provide several environment variables:
- APPLICATION_ID: the ID of your Discord bot application
- DISCORD_TOKEN: the Discord bot token acquired via the Discord Developer Portal
- MOD_ROLE_ID: the ID of the Moderator role on your Discord server (for `?cleanup`)
- RUSTACEAN_ROLE_ID: the ID of the Rustacean role on your Discord server (for `?rustify`)

An example command-line for Linux would be:
`APPLICATION_ID=812377560917868595 MOD_ROLE_ID=583178325221048320 RUSTACEAN_ROLE_ID=319953207193501696 DISCORD_TOKEN=... cargo run --release`

### Docker

This project has a Dockerfile, so you can use Docker to run this bot if you wish.
For that, rename the `.env.example` file into `.env`, fill out the values, and run the commands:

```sh
docker build -t rustbot -f Dockerfile .
docker run -d --rm --name rustbot --env-file .env rustbot
```

## Credits

This codebase has its roots in [rust-lang/discord-mods-bot](https://github.com/rust-lang/discord-mods-bot/), the Discord bot running on the official Rust server.
