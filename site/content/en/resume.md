A checkpoint continues explicit-root directory discovery after a traversal limit or interruption. It does not resume disk measurements, local hashing or review evidence.

## Save and continue

Replace the example root with your chosen absolute path. Keep output outside that root. Run the second command only when the first pass published a checkpoint and discovery remains incomplete.

```sh
scan_dir="$(mktemp -d /private/tmp/wt-sweep-discovery.XXXXXX)"
./target/release/wt-sweep scan --root /absolute/path/to/projects \
  --checkpoint-out "$scan_dir/pass-1.json" --run-dir "$scan_dir" --html

./target/release/wt-sweep scan --resume "$scan_dir/pass-1.json" \
  --checkpoint-out "$scan_dir/pass-2.json" --run-dir "$scan_dir" --html
```

The saved checkpoint supplies the original roots. Optional repeated roots must match exactly, including order. New output names prevent overwriting an earlier pass.

Only explicit-root `scan` supports checkpoints. `--repo`, `--all` and `review` do not accept them. A wide directory may need a higher allowed entry cap or a narrower fresh root.

## Interpret the next pass

Known family evidence is read again. Completed directories retain historical observations; a newly created repository inside one may not be discovered by a resume. Directory identities and checkpoint digests are validated. A changed identity can refuse the resume.

Read coverage, pending paths and publication omissions. Separate passes keep separate observation intervals and never form an atomic snapshot. Neither a checkpoint nor completion authorizes cleanup.
