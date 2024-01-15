#!/usr/bin/env bash
DB_PATH=~/.jstz/log.db

if [ ! -f $DB_PATH ]; then
    echo "Creating log database at $DB_PATH"
fi

cd $(dirname "$0")
sqlite3 $DB_PATH < create_log_db.sql