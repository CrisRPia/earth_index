# Earth Index

A PostgreSQL/PostGIS feature index for geographic screening and Earth–Moon–Mars
analog research. See [DESIGN.md](DESIGN.md) for the database specification.

The repository contains source discovery, terrain and temperature pilots, and a
working PostgreSQL/PostGIS import through Docker Compose. The API will be written
in **Rust**; no HTTP API or published scientific release exists yet.

## Local database with Docker

```sh
python3 scripts/setup_local_env.py
docker compose up -d --wait db
docker compose run --build --rm loader
docker compose run --rm --entrypoint python loader scripts/test_postgres.py
```

This loads **143,850 numeric observation slots** for 959 cells in three regions:
130,211 numeric values and 13,639 explicit missing values. All 91 candidates are
cataloged; 15 concrete pilot output definitions have data. An identical import
is reused rather than duplicated. The import is atomic and preserves source
manifests, native units, temporal windows and clipped cell support.

Measured storage is about **51 MB for the database**, including PostgreSQL/PostGIS
catalogs, with **31 MB of project tables and indexes**. The source pilot artifacts
occupy about **12.1 MB**, including **10.65 MB of CSVs**. No rasters are loaded.
Container images and the PostgreSQL cluster's WAL are separate disk costs.

The database listens on `127.0.0.1:5432` and persists in a named Docker volume.
Private local connection strings are in ignored `.env`; the Rust API should use
`API_DATABASE_URL`, whose role can read the `research` views. See
[database operations and Rust integration](docs/DATABASE.md) and the measured
[load report](research/pilots/database/load.json).

## Frontend catalog

The [complete variable list](docs/VARIABLES.md) covers 91 candidates across 17
groups. [Machine-readable definitions](catalog/variables.json) include stable
IDs, labels, proposed units, dimensions, source mappings and explicit pilot
availability. [TypeScript types](catalog/types.ts) and a
[real pilot observation example](catalog/observation-example.json) support
frontend development before the API is implemented.

Eight variables have processed pilot statistics; aspect has related components.
Other fields remain planned or deferred. This roadmap does not promise that all
candidates will pass source and scientific validation. Regenerate it with:

```sh
python3 scripts/export_frontend_catalog.py
```

## Discovery

The three pilot regions are 100 × 100 km squares in northern Chile, central
Iceland, and the central Sahara. Public catalog checks and a broad registry of
91 candidate feature definitions are saved under `research/`. A catalog hit does
not establish valid pixels or scientific usefulness. Current extraction progress
and remaining work are explicit in
[processing-status.json](research/pilots/processing-status.json).

```sh
uv run scripts/inspect_pilot_sources.py --cached
python3 scripts/inspect_candidate_sources.py
```

## Local terrain processing

```sh
uv run scripts/process_pilot_terrain.py
uv run scripts/test_pilot_terrain.py
python3 scripts/validate_pilot_terrain.py
```

The processor reads windows from public Copernicus GLO-30 Cloud Optimized
GeoTIFFs using HTTP range requests. No account, cloud compute, database server,
or persistent raster cache is required. Dependencies are pinned in the scripts;
`uv` keeps their environments outside the project. Run one region with
`--site atacama`, `--site iceland`, or `--site sahara`.

Outputs in `research/pilots/terrain/`:

- One CSV per region with 12 typed numeric summary fields per H3 resolution-6
  cell: elevation min/mean/max/population standard deviation; slope
  mean/standard deviation/max; mean 3×3 height roughness, topographic position,
  and local relief; and mean aspect eastness/northness components.
- Clipped cell geometries, source references, decoded-window hashes, support and
  quality metadata, and run/resource reports.
- `run.json` records runtime, peak process memory, and retained output size inputs.
  Decoded window bytes are not a measurement of HTTP transfer.

Boundary cells summarize only their intersection with the pilot square. Keep
that spatial support when importing them into PostgreSQL. CSVs are ingestion
artifacts, not an alternate live database.

Elevation statistics use native postings, fractional polygon/pixel overlap, and
spherical pixel-area weights. Derivatives use native 3×3 neighborhoods with
geodesic horizontal distances. Flat pixels have no aspect; asset-edge and nodata
neighborhoods are excluded and their missing support is reported. There is no
elevation reprojection or resampling.

Copernicus is a mixed-epoch **surface** model in metres relative to EGM2008;
these are not 2024 bare-ground observations. The public assets used here supply
no auxiliary editing/filling/error/water masks. Results currently apply nodata
screening only and require further scientific review before publication. Source
ETags are recorded as opaque identifiers, not assumed to be content checksums.

Source: [Copernicus product description](https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM).
Methods: [exactextract operations](https://isciences.github.io/exactextract/operations.html).

## Parallel NASA temperature processing

The completed temperature pilot uses original MOD11A2.061 HDF4 files for all
46 composite windows in 2024. Eight parallel downloads interleave all three
regions. HDF4 decoding runs one file at a time to avoid library races; only the
intersecting source window is decoded, then the temporary file is deleted.

```sh
uv run scripts/process_native_temperature.py --credentials-file /private/path/to/login
uv run scripts/test_native_temperature.py
uv run scripts/test_nasa_temperature.py
python3 scripts/validate_nasa_temperature.py --native
```

The measured run completed in **103.6 seconds**, used **314 MiB peak process
memory**, transferred **884 MB** of original payloads, and used at most **7.3 MB**
for a single scratch raster. No rasters remain. Summaries and provenance occupy
about **8.2 MB** under `research/pilots/temperature-native/`.

The 44,114 cell-window rows contain mean day/night temperatures, paired-pixel
day-minus-night differences, and separate valid-area fractions. There are
118,703 valid numeric values and 13,639 missing values after strict QA screening;
missingness is retained rather than imputed. See the
[method and results](research/pilots/temperature-native/README.md).

## Optional NASA server-side subsets

```sh
python3 scripts/prepare_nasa_subsets.py
```

This prepares one full-year 2024 MOD11A2.061 area request per pilot, in native
projection, with day/night temperatures, their QA fields, emissivities, and
clear-sky bitfields. It validates layer names against the captured live AppEEARS
product metadata. Preparing requests does not submit jobs or download rasters.
Other candidate NASA products have availability checks in
`research/pilots/appeears-products.json`; a product absent from AppEEARS may still
be available through another service. Refresh these public checks with
`python3 scripts/inspect_appeears_sources.py`.

NASA requires an [Earthdata account](https://urs.earthdata.nasa.gov/users/new).
The helper accepts a **private file outside the repository** containing
`username:password`; never put credentials in command arguments or tracked files.

```sh
python3 scripts/prepare_nasa_subsets.py --submit --credentials-file /private/path/to/login
python3 scripts/prepare_nasa_subsets.py --status --credentials-file /private/path/to/login
```

The helper logs in to AppEEARS, keeps the temporary token in memory, saves only
task receipts, and logs out. It does not forward credentials through redirects.
It avoids resubmitting a request with the same saved payload hash. Authentication
has been verified and all three pilot requests have been submitted. Request
submission is separate from completing NASA's server-side subset processing.

The in-memory consumer can inspect the first returned headers or aggregate all
completed regions. It reports queued regions explicitly; rerunning it reuses
completed summaries only when their code, geometry, task and CSV hashes match.

```sh
uv run scripts/process_nasa_temperature.py --inspect --credentials-file /private/path/to/login
uv run scripts/process_nasa_temperature.py --credentials-file /private/path/to/login
uv run scripts/test_nasa_temperature.py
python3 scripts/validate_nasa_temperature.py
```

Temperature artifacts live under `research/pilots/temperature/`. Each completed
region has one row per H3 cell and 8-day window, with mean day/night temperatures,
paired-pixel day-minus-night difference, and separate valid-area fractions.
The last leap-year 2024 composite is six days. Empty values remain missing.
The reader verifies NASA's SHA-256 hashes, decodes supported temperature storage
formats exactly once, checks native grid alignment, and keeps only QA byte zero
(good production/data flags, emissivity error ≤0.01, LST error ≤1 K). QA zero is
not treated as a missing value. Composite flags describe majority daily input
flags; they do not establish that every constituent daily retrieval met this QA.

Source pixels and signed S3 download URLs remain in memory and are discarded;
only summaries and whitelisted provenance are retained. Eight fixtures cover
unit conversion, missingness, QA, fractional overlap, paired support, temporal
windows, alignment, and credential stripping on signed redirects. The first
returned Atacama bundle included an extra 2023 date and lacked two daytime QA
exports. The consumer fails closed on incomplete layer groups; it does not infer
missing QA. The direct native-file path above supplies the completed, validated
temperature outputs independently of these jobs. The AppEEARS downloader also
rejects redirect hosts outside its configured AWS allowlist, so this optional
path may need adaptation to current service routing.
Emissivity and clear-sky layers were requested for later validation but are not
yet included in the temperature summary CSVs. These are research artifacts;
These numeric pilots are now loaded through Docker Compose; scientific
publication and the Rust API remain subsequent steps.

API: [AppEEARS documentation](https://appeears.earthdatacloud.nasa.gov/api/).
