# Native Local AI in v0.5

Open **Settings → Local AI**. Select an official pinned 0.6B or 1.7B model,
review model/runtime sizes and license/resource guidance, then explicitly approve
the download. Native Rust downloads and verifies both artifacts, supports cancel
and safe restart after interruption, and stores them under `local-ai` in the OS
user-data directory. Installed Atlas does not need Python. Verify the model,
enable it and use **Test model** for an actual measured local response. A checksum
configuration alone is not a successful inference benchmark.

Verification derives installed runtime bytes from the pinned upstream archive;
editable cache inventory cannot replace catalog hashes. Changing model files is
serialized against inference. Threads (1–4), context (1024–8192), output cap
(64–1024) and idle shutdown (30–600 seconds) are bounded. Only one inference job
runs at a time. Disabling/stopping AI leaves ordinary planning available.

The Assistant offers **Atlas Planner** and **Local AI**. AI explanations use
saved planner results, run through the authenticated loopback runtime and have
no command/file/network tools or database-write authority. Proposed schedules
are reviewed separately in Map. Exact money/time calculations remain in Rust.

The cloud native download attempt remains blocked; see [v0.5 validation](VALIDATION-v0.5.md)
for actual local and Windows evaluation outcomes. The catalog provenance below
is retained. The Python helper remains optional for developers.

# Optional local inference

Atlas runs without a model. Ordinary parsers, calendar records, financial
calculations, reminders, and file import do not depend on inference. No model
or executable is downloaded automatically or bundled in application updates.

The implemented adapter supervises a separately installed `llama-server`.
It launches only after the executable and GGUF SHA-256 digests match the
configured expected hashes. It binds a dynamically allocated loopback port,
generates an in-memory authentication token, disables the server web UI,
allows one inference job, limits generation and prompt processing to two CPU
threads, uses a 4,096-token context and 700 output-token limit, and terminates
the owned process after 120 idle seconds. Cancellation terminates that owned
process. Process output and source text are not written to diagnostic logs.

Model extraction uses constrained JSON with explicit nulls and non-thinking
chat-template parameters. Application validation checks source snippets,
dates, date ordering, IANA timezones, exact monetary amounts, and allowable
fields. Unsupported values are replaced with null and reported for review.
Facilities remain unknown. Model output never authorizes a shell command,
network fetch, purchase, cancellation, or database write. A review draft is
the output; a model-generated confidence number is not accepted as evidence.

## Configure a verified installation

1. Obtain a pinned llama.cpp release or build a pinned reviewed commit from
   <https://github.com/ggml-org/llama.cpp>. Verify the upstream release digest or
   source provenance before recording the executable's SHA-256. Do not replace
   an expected hash merely to make an integrity check pass.
2. Inspect the official model repository, exact revision, license, filenames,
   sizes, and SHA-256/LFS object metadata before downloading weights. A
   quantized GGUF must have its own verified source and checksum; the existence
   of original Qwen weights does not prove a particular four-bit file exists.
   If converting the official model, pin the converter/runtime version and
   record the input revision/digests, conversion command, output digest and
   license together in a private model inventory.
3. Review the download size before initiating it. Keep models outside the
   program directory and source checkout, for example in the OS user-data
   model-cache directory. Runtime memory is larger than download size and
   depends on quantization, context, and implementation.
4. Set local application settings under `ai` (paths are machine-local):

   ```json
   {
     "enabled": true,
     "modelName": "the exact verified model and quantization",
     "runtimePath": "an absolute path to llama-server",
     "runtimeSha256": "the independently verified 64-character SHA-256",
     "modelPath": "an absolute path to the GGUF file",
     "modelSha256": "the independently verified 64-character SHA-256",
     "modelSource": "https://the-authoritative-source/exact-revision",
     "modelLicense": "the verified model license"
   }
   ```

5. Run `ai.extract` with a synthetic confirmation, review the draft, and use the
   returned elapsed milliseconds and usage counts to measure this machine.
   Test cancellation and observe idle process termination. Benchmark on the
   actual target PC before claiming acceptable CPU latency or RAM use.

`ai.status` reports configured/available/running separately. A configured hash
is not a verified file until startup actually checks it. `ai.cancel` kills only
the inference process that Atlas launched. Saved configuration can be disabled
without losing records. OCR for scanned PDFs and images is not implemented.

## Verified artifact catalog

On 2026-10-22, the official Qwen model APIs, pinned repository trees, model card,
and Apache 2.0 license were retrieved over verified HTTPS. Both official GGUF
repositories contained **Q8_0 only**; no four-bit artifact is assumed. The
official model card documents non-thinking mode and llama.cpp compatibility.

| Candidate | Pinned official repository revision | Download bytes | Official LFS SHA-256 |
| --- | --- | ---: | --- |
| Qwen3-1.7B-Q8_0 | `Qwen/Qwen3-1.7B-GGUF` at `90862c4b9d2787eaed51d12237eafdfe7c5f6077` | 1,834,426,016 | `061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a` |
| Qwen3-0.6B-Q8_0 | `Qwen/Qwen3-0.6B-GGUF` at `23749fefcc72300e3a2ad315e1317431b06b590a` | 639,446,688 | `9465e63a22add5354d9bb4b99e90117043c7124007664907259bd16d043bb031` |

Sources: [1.7B model](https://huggingface.co/Qwen/Qwen3-1.7B-GGUF/tree/90862c4b9d2787eaed51d12237eafdfe7c5f6077),
[0.6B model](https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/tree/23749fefcc72300e3a2ad315e1317431b06b590a),
and [pinned model license](https://huggingface.co/Qwen/Qwen3-1.7B-GGUF/blob/90862c4b9d2787eaed51d12237eafdfe7c5f6077/LICENSE).

The official llama.cpp **b11827** release was pinned to commit
`970b51ce7c9ba5fff0726c5e9994f1c2f0e4eedc`. The Linux x64 CPU archive,
`llama-b11827-bin-ubuntu-x64.tar.gz` (22,343,386 bytes), was downloaded and
verified against the GitHub release API digest:
`7c1de2b2a139aa90e28b88d44a75f5966485604f68aa0ccdc2339959bca9b5f6`.
The extracted `llama-server` executable has SHA-256
`92ff7abb8a59c94a98acdbca4f50aaf54d2cb08a05fad57b254587a101366aac`.
Its `--version` and `--help` executed successfully on the cloud Linux machine.
The installed distribution includes dependent libraries; preserve and verify
the complete release archive, not an executable copied away from its libraries.

Pinned Windows CPU x64 and macOS arm64/x64 release archive digests are recorded
in `scripts/install-local-ai.py` from the same official release API. Those
platform artifacts have **not** been executed here. Source:
<https://github.com/ggml-org/llama.cpp/releases/tag/b11827>.

The actual runtime help confirms `--threads`, `--threads-batch`, `--parallel`,
`--ctx-size`, `LLAMA_API_KEY`, `--no-webui`, and `--sleep-idle-seconds`. The
official server documentation describes JSON-schema responses and
`chat_template_kwargs` with `enable_thinking: false`:
<https://github.com/ggml-org/llama.cpp/blob/970b51ce7c9ba5fff0726c5e9994f1c2f0e4eedc/tools/server/README.md>.

## Reproduce installation and measurement

The optional developer helper requires **Python 3.12 or newer** for
`hashlib.file_digest` and safe tar extraction filters. It prints model/runtime
sizes without downloading.
Review that output before adding `--download`. It uses normal TLS verification,
rejects HTTPS-to-HTTP redirects before issuing the redirected request, checks
exact pinned sizes and SHA-256 hashes, and refuses mismatching existing
files. It saves the model, runtime, inventory and machine-local settings outside
the source checkout. Installed Atlas itself does not need Python or this helper.

```sh
python3 scripts/install-local-ai.py --model 1.7b --cache /absolute/private/atlas-models
python3 scripts/install-local-ai.py --model 1.7b --cache /absolute/private/atlas-models --download
cargo run -p atlas-core --example benchmark_ai -- /absolute/private/atlas-models/atlas-ai-settings.json --idle
```

The benchmark uses only three synthetic confirmations (complete, missing facts,
and revised) through the real `ai::extract` service. JSON results contain cold
and warm wall time, usage counts, retained/rejected fields, peak child RSS on
Linux, cancellation results, and optional idle process shutdown. Performance
applies only to the measured hardware and fixtures. The cloud machine reports
an AMD EPYC 9V74 host, a four-CPU cgroup quota, and a 32 GiB memory cgroup limit;
the inference configuration remains capped at two processing/generation threads.

Current blocker: model metadata is accessible, but the GGUF download redirects
to `us.aws.cdn.hf.co`, where the active cloud egress proxy returned CONNECT HTTP
403. That exact CDN hostname was added to the saved environment configuration
draft while preserving existing destinations; draft saving did not establish
active access. The actual model bytes have **not** been verified locally or
loaded, and no model inference, CPU latency, token-rate, or peak-RAM result is
claimed. After active egress permits the CDN, run the commands above and record
their actual results. No default model is enabled pending that evaluation.

The adapter's passing evidence/schema/checksum tests and the compiled benchmark
harness establish code behavior, not successful inference. No OCR for scanned
documents or images is implemented. These limitations do not affect ordinary
parsers, calendar, budgeting, or reminders.
