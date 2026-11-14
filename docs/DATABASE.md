# PostgreSQL pilot database

The database runs in Docker Compose with a digest-pinned PostgreSQL 18/PostGIS
image. It contains research pilot data. The application API will be written in
Rust; the ingestion loader remains Python to work with the scientific pipeline.

## Start and load

Requirements: a running Docker engine and Docker Compose. Rootless Docker works.

```sh
python3 scripts/setup_local_env.py
docker compose up -d --wait db
docker compose run --build --rm loader
docker compose run --rm --entrypoint python loader scripts/test_postgres.py
```

The setup helper generates private random passwords in `.env` with mode 0600 and
preserves an existing file. Never commit that file. To use a different host port,
adjust `POSTGRES_PORT` and the two host connection strings together.

`db` publishes only `127.0.0.1:5432`. The loader connects to `db:5432` on the
Compose network. PostgreSQL data persists in `earth-index_postgres_data`, mounted
at `/var/lib/postgresql`, as required by the
[PostgreSQL 18 PostGIS image](https://github.com/postgis/docker-postgis#%EF%B8%8F-pgdata-volume-path-change).
The host source directory is read-only inside the loader; only its `/reports`
mount is writable. No raster source files are loaded.

```sh
docker compose ps
docker compose stop db
docker compose start db
docker compose down
```

Stopping containers or running `down` preserves the named volume. `down -v`
deletes the database volume; use it only when deliberately discarding local data.
The `.env` passwords initialize an empty database. Editing the file does not
rotate passwords in an existing volume.

## Contents and size

- 91 candidate quantity definitions; 15 realized numeric output definitions.
- 959 full H3 resolution-6 cells and their clipped supports in three pilot regions.
- 45 series: 12 terrain outputs × 3 regions, plus 3 temperature outputs × 3 regions.
- 47 distinct time windows: one static window and 46 MOD11A2 composites in 2024.
- 143,850 observation rows: 130,211 numeric values and 13,639 SQL NULL values with
  explicit missing reasons. Zero is preserved as a valid value.
- Source manifests, original source URLs and hashes, QA semantics, units,
  grid/operator versions, support and coverage fractions.

The source terrain and temperature artifacts occupy approximately 12.1 MB,
including 10.65 MB of CSVs. The initial database measured approximately 51 MB;
project tables including indexes and TOAST occupied 31 MB. Database size includes
PostgreSQL/PostGIS catalogs but excludes cluster WAL, other cluster databases,
Docker images, build caches and the source CSV files. These figures describe only
the three pilot regions. The [load report](../research/pilots/database/load.json)
records byte counts and table sizes; normal database operations can change them.
The local PostgreSQL volume measured about 195 MB including cluster-wide WAL,
system databases and other cluster files. This is separate from both the 51 MB
database measurement and the Docker image/build storage. A separate
[storage report](../research/pilots/database/storage.json) records these scopes.

## Import guarantees and scope

`scripts/load_pilots.py` verifies scientific code, inventory, geometry and CSV
hashes before writing. A transaction-scoped advisory lock serializes imports and
migrations. Migrations are ordered and checksum-checked. One transaction creates
the catalog, support geometry, provenance, series and observations. Failure
rolls back all project changes. A repeated bundle hash reuses the existing import
and checks its row counts instead of duplicating observations.

Numeric values must be finite, valid fractions must be in [0,1], missing rows
must have a reason and zero valid support, and usable values must have positive
support. Foreign keys enforce series/grid/region/cell identity. Static and
interval windows are distinct. Sealed series and observations reject mutation;
versioned metadata is append-only. The owner remains a development administrator,
not a production ingestion role.

Some raw HDF4 header strings contain U+0000, which PostgreSQL JSONB rejects.
Those strings are stored reversibly as a tagged object with `code_points` and
`earth_index_encoding`. Original manifests remain unchanged in the repository and
their SHA-256 hashes are stored. Do not treat encoded header strings as numeric
quality values. Scientific raster decoding is unchanged.

Integration checks compare every stored value, coverage fraction and unit against
the source CSVs, verify missingness and boundary/time semantics, reject invalid
values and mutation of sealed records, and exercise the API login permissions.
Fixture writes are rolled back. The first actual failed load also verified that
no observation table survived the transaction rollback.

This pilot migration stores numeric observations only. Categorical/boolean
observations, normalized source-variable/asset relations, published releases,
wide release projections, arbitrary SQL execution controls and query audit
records remain to be implemented from DESIGN.md. Candidate variables do not
imply globally available observations or a production scientific release.

## Rust API integration

Use `.env`'s `API_DATABASE_URL` for the Rust service. `DATABASE_URL` is the local
owner connection for administration/ingestion; if the Rust database driver
expects `DATABASE_URL`, supply the API URL as that service's environment value.
Do not pass the owner credentials to the application. No Rust service has been
implemented yet.

The `earth_index_api` login inherits SELECT access to research views, defaults to
read-only transactions and has a 10-second statement timeout. It has no access to
underlying project schemas or writes. These permissions protect the local pilot;
they are not the future arbitrary research-SQL execution sandbox.

Available views:

| View | Purpose |
| --- | --- |
| `research.variables` | All 91 candidate definitions, labels and frontend metadata |
| `research.pilot_features` | Realized output IDs, units, definitions and series |
| `research.pilot_observations` | Sealed numeric pilot values, missingness, periods, fractions and provenance IDs |
| `research.cells` | Full H3 geometry, center and WGS84 ellipsoidal area |
| `research.cell_supports` | Region-clipped support geometry and area metadata |

Use parameterized SQL. For example, a Rust handler can bind a region key, output
ID and time range to the following query, then map the SQL names to the frontend
[observation contract](../catalog/types.ts):

```sql
SELECT variable_id, output_id, region_id_external, cell_id,
       value_numeric, unit, missing_reason, valid_area_fraction,
       time_kind, time_start, time_end_exclusive, run_id, bundle_sha256
FROM research.pilot_observations
WHERE region_id_external = $1
  AND output_id = $2
  AND time_start >= $3::timestamptz
  AND time_start < $4::timestamptz
ORDER BY time_start, cell_id;
```

For static terrain, use `time_kind = 'static'` instead of inventing timestamps.
The temperature difference is paired-pixel composite day minus night, not a
full diurnal amplitude. Kelvin differences must not have 273.15 subtracted.
Boundary cells summarize clipped geometry; map the support geometry from
`research.cell_supports` when presenting coverage. Do not label clipped coverage
as full-cell coverage.

For local administration without installing `psql` on the host:

```sh
docker compose exec db psql -U earth_index_owner -d earth_index
```

The container's local administrative socket is intentionally for development.
Production deployment still needs separate migration/ingestion identities,
connection encryption, backups, restore testing and release publication checks.

On this workstation, Docker was installed using the official
[rootless installation](https://docs.docker.com/engine/security/rootless/), with
the `rootless` context selected. Its user service can be controlled with
`systemctl --user start docker` or `systemctl --user stop docker`.
