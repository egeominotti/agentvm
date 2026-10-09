// S3-compatible providers and how each wants to be addressed.

export type Preset = { label: string; endpoint: string; region: string; pathStyle: boolean; hint: string };

export const PRESETS: Record<string, Preset> = {
  aws: {
    label: "AWS S3",
    endpoint: "https://s3.eu-central-1.amazonaws.com",
    region: "eu-central-1",
    pathStyle: false,
    hint: "Endpoint s3.<region>.amazonaws.com.",
  },
  r2: {
    label: "Cloudflare R2",
    endpoint: "https://<account-id>.r2.cloudflarestorage.com",
    region: "auto",
    pathStyle: true,
    hint: "Region is always auto. Use an R2 API token's access key and secret.",
  },
  hetzner: {
    label: "Hetzner",
    endpoint: "https://fsn1.your-objectstorage.com",
    region: "fsn1",
    pathStyle: false,
    hint: "Locations: fsn1, nbg1, hel1. The region is the location.",
  },
  b2: {
    label: "Backblaze B2",
    endpoint: "https://s3.eu-central-003.backblazeb2.com",
    region: "eu-central-003",
    pathStyle: false,
    hint: "Use an application key; the region is in the endpoint.",
  },
  local: {
    label: "Local (RustFS / MinIO)",
    endpoint: "http://127.0.0.1:9100",
    region: "us-east-1",
    pathStyle: true,
    hint: "Start it with scripts/dev-s3.sh up. Access key agentvm, secret agentvm-local-secret.",
  },
};
