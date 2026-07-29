CREATE TABLE example_records (
    id integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    content text NOT NULL
);

INSERT INTO example_records (content) VALUES ('pack_database_marker');
