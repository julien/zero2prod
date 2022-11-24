#!/usr/bin/env bash
# make sure you start postgres before running migrations, etc...!
set -x 
set -eo pipefail
export DATABASE_URL=postgres://postgres:password@127.0.0.1:5432/newsletter
