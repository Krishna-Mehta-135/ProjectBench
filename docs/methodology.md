# Methodology

ProjectBench results are self-measured on local Docker Compose, not an independent audit. The host shares resources between the application and generator, and Docker Desktop runs containers in a virtual machine on macOS. Absolute times therefore do not represent production latency.

## Comparative verdicts

Each workload needs the configured minimum repetitions for both variants. The environment must match across variants. ProjectBench compares medians and reports min–max ranges. If either observed range width exceeds the absolute difference between medians, the verdict is inconclusive. A generator CPU peak above 70% or mismatched environments also makes the comparison inconclusive. These fixed rules avoid presenting noisy or unfair results as supported claims.

## Behavior verdicts

Behavior claims compare every observed metric with a minimum expected value. A result is verified only when the required observations satisfy that threshold. Otherwise it is not supported or inconclusive according to the stored data.

## Limitations

Results say what the stated workload observed under the recorded environment. They do not establish production capacity, behavior under every workload, or general performance across machines. Publish the workload, metric definition, environment, samples, and limitations together.
