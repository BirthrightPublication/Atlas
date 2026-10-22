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

## Evidence and current limits

On 2026-10-22, the official Qwen3 GitHub README was retrieved over verified
HTTPS. It lists dense 0.6B and 1.7B models, thinking/non-thinking modes, and
Apache 2.0 model licensing with the actual license in each model repository:
<https://github.com/QwenLM/Qwen3>.

The official llama.cpp server documentation was retrieved over verified HTTPS.
It documents `--threads`, `--threads-batch`, `--parallel`, `--ctx-size`,
`--api-key` / `LLAMA_API_KEY`, `--no-webui`, `--sleep-idle-seconds`, JSON-schema
responses, and `chat_template_kwargs` including `enable_thinking: false`:
<https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md>.

The cloud network proxy returned HTTP 403 for
<https://huggingface.co/Qwen/Qwen3-1.7B>. Exact official weights, revision,
license file, size, checksum and a compatible quantized artifact could not be
verified here. No model was selected, downloaded, or benchmarked; no measured
CPU/RAM claim is made. The 1.7B model and smaller 0.6B alternative remain
candidates for evaluation. This limit does not affect deterministic imports.

The adapter's evidence/schema/checksum tests validate code behavior; they do
not establish a successful model inference. A verified model installation and
target-machine benchmark remain required before calling the AI feature ready.
