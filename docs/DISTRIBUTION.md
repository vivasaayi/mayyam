# Distribution guide

There are two supported runtime modes now:

1. `local`: full local stack for development and integration tests.
2. `distributable`: packaged Mayyam with its own internal app database, plus optional real Kafka/MySQL/Postgres/AWS targets.

Local mode

```bash
bash scripts/bootstrap.sh local up
bash scripts/bootstrap.sh local test
bash scripts/bootstrap.sh local down
```

What local mode includes:
- PostgreSQL for Mayyam app data
- MySQL for local target testing
- Kafka + Zookeeper
- LocalStack for AWS-style local integration tests
- Backend dev container with hot reload
- Frontend dev container with hot reload
- Integration test container

Distributable mode

`docker-compose.distributable.yml` always starts an internal PostgreSQL database for Mayyam itself. That internal DB is separate from any real PostgreSQL/MySQL/Kafka/AWS systems that users want Mayyam to connect to.

Bootstrap steps:

```bash
cp .env.distributable.example .env.distributable
bash scripts/bootstrap.sh distributable up
```

Before starting, edit `.env.distributable` with the real targets you want Mayyam to connect to:
- `TARGET_POSTGRES_*` for a real PostgreSQL instance
- `TARGET_MYSQL_*` for a real MySQL instance
- `TARGET_KAFKA_*` for a real Kafka cluster
- `AWS_*` and `TARGET_AWS_*` for real AWS access

The bootstrap script generates `config.distributable.yml` from `.env.distributable` and mounts it into the container as Mayyam's runtime config.

Remote install

For remote machines without a repo checkout:

```bash
curl -sSL https://raw.githubusercontent.com/sumitharajan/mayyam/main/scripts/install.sh | bash
```

That installer now:
- downloads `docker-compose.distributable.yml`
- creates `.env.distributable`
- generates `config.distributable.yml`
- starts Mayyam plus its internal app DB

Image publishing example

```bash
docker build -t your-dockerhub-username/mayyam:1.0.0 .
docker login
docker push your-dockerhub-username/mayyam:1.0.0
```

Then update `MAYYAM_IMAGE` in `.env.distributable`.
