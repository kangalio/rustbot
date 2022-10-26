-- Add migration script here
CREATE TABLE ub (
    time TEXT NOT NULL,
    channel INTEGER NOT NULL,
    kind TEXT NOT NULL,
    UNIQUE (channel, kind)
);

CREATE INDEX ub_channel_kind_idx ON ub(channel, kind);