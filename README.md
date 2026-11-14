# Earth Index

A PostgreSQL/PostGIS feature index for geographic screening and Earth–Moon–Mars
analog research. See [DESIGN.md](DESIGN.md) for the database specification.

The repository currently contains source discovery and a working local terrain
pilot. It does not yet contain a running PostgreSQL database or a published
scientific release.

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

## NASA temperature subsets

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
It avoids resubmitting a request with the same saved payload hash. A submitted
request still needs output retrieval, product-specific QA/scale validation, and
tile aggregation; those temperature ingestion stages are not implemented yet.
No NASA temperature values have been imported merely by preparing a request.

API: [AppEEARS documentation](https://appeears.earthdatacloud.nasa.gov/api/).
