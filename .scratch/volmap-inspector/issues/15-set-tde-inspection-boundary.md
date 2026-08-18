Type: grilling
Status: open
Blocked by: 02

# Set the TDE-encrypted page inspection boundary

## Question

What should version one do when the plaintext I/O-page envelope marks its 16,344-byte user region as AES- or ARIA-encrypted? Choose between an intentionally opaque metadata-only classification, optional decryption supplied through a narrowly defined key interface, or another bounded scope. Account for strict offline operation, no CUBRID runtime-library dependency, key custody and zeroization, remote web exposure, raw-byte policy, fixture availability, and the rule that ciphertext must never be parsed as page structures. Define the precise CLI/JSON/TUI/web behavior and whether decryption is a release blocker or future work.

## Comments
