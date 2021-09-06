-- Add migration script here
CREATE TABLE showcase (
    output_message INTEGER NOT NULL PRIMARY KEY,
    output_channel INTEGER NOT NULL,
    input_channel INTEGER NOT NULL,
    name_input_message INTEGER NOT NULL,
    description_input_message INTEGER NOT NULL,
    links_input_message INTEGER NOT NULL
)
