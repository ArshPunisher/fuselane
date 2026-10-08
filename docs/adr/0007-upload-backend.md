# 0007. Cloudflare Workers + R2 + D1 for uploads and share links
- Status: Proposed — confirmed or rejected by spike S6
- Date: 2026-10-08

## Context
A sharing product lives or dies on egress cost. AWS S3 egress is about $0.09/GB. R2 egress is $0. Uploads must go straight to storage (Worker request-body limits).

## Decision
A Worker (Hono) handles the control plane: create, presign parts, list parts, complete, share pages, expiry cron. D1 stores uploads and shares. R2 stores objects. Clients PUT parts directly to R2 with presigned URLs.

## Consequences
- R2's equal-part-size rule: parts never split dynamically.
- Vendor lock-in is limited to the control plane, since R2 speaks S3.
- Bring-your-own S3 bucket comes later, through the same client.
