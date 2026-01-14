# Dev DB (orsx2)

Starts a dedicated PostgreSQL instance for `orsx2` integration tests/benchmarks.

## Start

```bash
docker compose -f dev/docker-compose.yml up -d
```

Wait for healthy:

```bash
docker ps --filter name=orsx2-test-db
docker logs -f orsx2-test-db
```

## Connection string

```bash
export ORSX_TEST_DATABASE_URL="postgresql://orsx:orsx@localhost:15432/orsx2_test"
```

## Stop / cleanup

```bash
docker compose -f dev/docker-compose.yml down
```

Delete data volume (destructive):

```bash
docker compose -f dev/docker-compose.yml down -v
```

