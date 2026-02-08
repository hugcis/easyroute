#!/bin/bash
set -e

# Create test database if it doesn't exist
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    SELECT 'CREATE DATABASE easyroute_test'
    WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'easyroute_test')\gexec

    GRANT ALL PRIVILEGES ON DATABASE easyroute_test TO easyroute_user;
EOSQL

echo "Test database 'easyroute_test' created/verified"
