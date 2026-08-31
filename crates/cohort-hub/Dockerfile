# Build from the repository root:
#   docker build -f crates/cohort-hub/Dockerfile -t cohort-hub .
# Run in a data centre:
#   docker run -p 7400:7400 -v cohort-data:/data \
#     -e COHORT_BIND=0.0.0.0:7400 -e COHORT_DB=/data/cohort.db \
#     -e COHORT_ALLOWED_ORIGINS=tauri://localhost,http://tauri.localhost \
#     cohort-hub

FROM rust:1.88-slim AS build
WORKDIR /src
COPY Cargo.toml ./
COPY crates ./crates
# Only the hub and its path dependencies are needed; strip the app member.
RUN sed -i 's#, "app/src-tauri"##' Cargo.toml \
    && cargo build --release -p cohort-hub

FROM debian:bookworm-slim
RUN useradd -r -u 10001 cohort && mkdir /data && chown cohort /data
COPY --from=build /src/target/release/cohort-hub /usr/local/bin/cohort-hub
USER cohort
ENV COHORT_BIND=0.0.0.0:7400 COHORT_DB=/data/cohort.db
EXPOSE 7400
CMD ["cohort-hub"]
