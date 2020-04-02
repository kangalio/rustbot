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

+ `MOD_ID` is the id of the mod role
+ `TALK_ID` is the id of the talk role
+ `DISCORD_TOKEN` is the token used to connect to discord
+ `DATABASE_URL` is the url where the database is running, you will need to
  update the port to the port your instance of postgres is running on

Once you have your guild setup, you can run the bot
```sh
docker run -e "MOD_ID=" -e "TALK_ID=" -e "DISCORD_TOKEN=" -e "DATABASE_URL=postgres://docker:docker@172.17.0.1:32768" --add-host=database:172.17.0.2 --rm -it discordbot
```
