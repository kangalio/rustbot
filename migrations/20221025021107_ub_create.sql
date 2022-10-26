CREATE TABLE ub (
    -- The previous time UB was invoked, will be in RFC 3339 format in UTC
    time TEXT NOT NULL,
    -- Channel ID where it was invoked. Should always be BEGINNER_CHANNEL_ID set from env,
    -- but this allows the channel to be updated.
    channel INTEGER NOT NULL,
    -- The kind of UB invoked
    kind TEXT NOT NULL,
    UNIQUE (channel, kind)
);

CREATE INDEX ub_channel_kind_idx ON ub(channel, kind);