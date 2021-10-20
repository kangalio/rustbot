CREATE TABLE last_godbolt_update (
    -- This is how we ensure uniqueness within this table, there
    -- should only ever be one single row (or zero rows, if the
    -- database was just created) within the table
    id INTEGER PRIMARY KEY CHECK (id = 0),
    -- This is the utc timestamp of the last time we updated the godbolt targets list
    last_update INTEGER NOT NULL
);

CREATE TABLE godbolt_targets (
    id TEXT UNIQUE NOT NULL,
    name TEXT UNIQUE NOT NULL,
    -- Lang should always be "rust", but I've kept the column
    -- just in case godbolt decides to do something interesting
    -- with it or we want to re-use this table for alternative
    -- compilers (DSLs *in* rust, maybe?)
    lang TEXT NOT NULL,
    compiler_type TEXT NOT NULL,
    semver TEXT NOT NULL,
    instruction_set TEXT NOT NULL,
    PRIMARY KEY (id, name)
);
