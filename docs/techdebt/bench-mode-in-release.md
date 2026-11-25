# Bench mode is in release builds

`KH_CSV` and `KH_BENCH_OUT` are read from the environment and the bench commands can be called by the window.
Using it needs control of the person's environment. Put it behind a `bench` build feature and build the smoke test
with that feature.
