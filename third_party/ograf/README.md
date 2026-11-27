# EBU OGraf v1 snapshot

Source: https://github.com/ebu/ograf
Commit: `945f8bbcbcba262377544871dd4af62e54de5177` (short pin `945f8bb`).

Copied directly from git blobs, without reformatting:
- `LICENSE` (MIT, Copyright 2026 European Broadcasting Union).
- `v1/specification/json-schemas/` (all schemas and its README).
- `v1/examples/` (all nine examples, including their assets and embedded notices).
- `v1/specification/open-api/server-api.yaml` (for the later API conformance gate).

The examples retain upstream's bundled dependencies and their embedded copyright/licence notices,
including Lottie and GSAP; the EBU licence does not replace those notices. No upstream development
build products, node_modules, specification diagrams or generated API documentation are copied.

`.gitattributes` disables text conversion for this snapshot. Do not edit these files to make a test
pass: change the engine or deliberately update the pin in a reviewed change (E-024). The engine
embeds the schemas at build time and resolves their canonical identifiers from these bytes only;
those identifiers are not network requests.
