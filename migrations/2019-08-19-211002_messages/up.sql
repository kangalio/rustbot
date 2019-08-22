-- Your SQL goes here
CREATE TABLE IF NOT EXISTS messages (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  message TEXT NOT NULL,
  channel TEXT NOT NULL
);
