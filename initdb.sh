#!/usr/bin/env bash

# Install the sqlx-cli tool with:
# cargo install --version=0.6 sqlx-cli --no-default-features --features postgres
# and make sure you start postgres and redis before creating the database and running migrations.
 
set -Euo pipefail
# For postgres use:
export DATABASE_URL=postgres://postgres:password@127.0.0.1:5432/newsletter

sqlx database create
sqlx migrate run

# When running tests you might need to use ulimit -n 102400
# to increase the limit of files that can be open.
