To update Zaino's doc pages:

1. Build docs on release branch using `$ cargo doc --workspace --no-deps`.

2. Replace files in this branch with docs generated in target/doc/*.

*NOTE: ./index.html is not generated and should be kept / copied from the current version unless changes are required.
