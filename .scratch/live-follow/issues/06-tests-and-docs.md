# 06 — End-to-end proof and docs

Blocked by: 03, 04, 05

- HTTP integration test: build a synthetic volume, boot `serve` on
  `127.0.0.1:0` with a short poll interval, fetch a live entity URL, mutate the
  volume, assert the generation advances on `/api/v1/live/watch` and that the
  same live URL still resolves. Use a minimal `std::net::TcpStream` HTTP client;
  the release graph stays pinned.
- Test that `--no-follow` still reaches the terminal invalidated snapshot.
- Update `README.md` web-access section. (`CONTEXT.md` vocabulary is done:
  source mode, snapshot generation, input fingerprint manifest, torn and
  superseded generation, live follow, generation retention window, and
  observed disk state.)
- `just verify`.
