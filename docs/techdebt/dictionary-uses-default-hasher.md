# Dictionary columns use the standard library hasher

`kh-core` dictionary columns use `std::collections::HashMap` (SipHash). It is safe against crafted collisions and
fast enough for the test files (1,000,000 rows read in 0.65 s). A faster hasher might help on the reference
machine. Measure there before changing it. Dictionary size is capped at 4,096 values, which bounds the cost of
a hostile file.
