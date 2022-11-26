#!/usr/bin/env bash

# Install the sqlx-cli tool with:
# cargo install --version=0.5.7 sqlx-cli --no-default-features --features postgres
# and make sure you start postgres before creating the database and running migrations.
 
set -Euo pipefail
# For postgres use:
export DATABASE_URL=postgres://postgres:password@127.0.0.1:5432/newsletter

sqlx database create
sqlx migrate run
