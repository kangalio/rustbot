-- Your SQL goes here
CREATE TABLE IF NOT EXISTS bans (
  id SERIAL PRIMARY KEY,
  user_id TEXT NOT NULL,
  guild_id TEXT NOT NULL,
  unbanned BOOLEAN NOT NULL DEFAULT false,
  start_time TIMESTAMP NOT NULL,
  end_time TIMESTAMP NOT NULL
);
