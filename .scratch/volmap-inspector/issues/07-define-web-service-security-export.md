Type: grilling
Status: open
Blocked by: 03, 04, 14

# Define the web service, remote-access, and HTML-export contract

## Question

What exact browser-facing contract should `volmap serve` and `volmap html` provide? Decide lazy read-only JSON resources, navigation URLs and stable identities, server lifecycle, default loopback binding, explicit `0.0.0.0` binding, mandatory non-loopback access tokens, origin and request protections, volume-path redaction, bounded raw-hex authorization, snapshot-change behavior, and the version-one boundary between a compact self-contained HTML report and details available only from the live inspector process. Assume no built-in TLS and document the trusted-network, SSH, VPN, or reverse-proxy boundary clearly.

## Comments
