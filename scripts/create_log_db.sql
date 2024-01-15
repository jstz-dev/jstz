CREATE TABLE request (
    id TEXT NOT NULL PRIMARY KEY,
    function_address TEXT NOT NULL
);

CREATE TABLE log (
    id INTEGER PRIMARY KEY,
    level TEXT,
    content TEXT,
    request_id TEXT NOT NULL,
        FOREIGN KEY (request_id) REFERENCES request (id)
);