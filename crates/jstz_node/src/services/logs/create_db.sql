CREATE TABLE IF NOT EXISTS request (
    id TEXT NOT NULL PRIMARY KEY,
    function_address TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS log (
    id INTEGER PRIMARY KEY,
    level TEXT,
    content TEXT,
    function_address TEXT NOT NULL,
    request_id TEXT NOT NULL,
        FOREIGN KEY (request_id) REFERENCES request (id)
);