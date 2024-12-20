## Postgres docker

`docker build -f postgres.Dockerfile -t trade-with-me-postgres .`

`docker run --name trade-with-me-postgres -p 5432:5432 -d trade-with-me-postgres`


## DB migrations

`./scripts/migration_up.sh`