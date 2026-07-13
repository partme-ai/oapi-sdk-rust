# Official SDK compatibility matrix

| Capability | Go | Python | Java | Rust 0.1 |
|---|---:|---:|---:|---:|
| Configurable API base URL | ✓ | ✓ | ✓ | ✓ |
| Custom HTTP client | ✓ | ✓ | ✓ | ✓ |
| Request timeout | ✓ | ✓ | ✓ | ✓ |
| Default headers / source | ✓ | ✓ | ✓ | ✓ |
| App access token | ✓ | ✓ | ✓ | ✓（商店应用需传 app_ticket） |
| Tenant access token | ✓ | ✓ | ✓ | ✓（商店应用需传 tenant_key） |
| User token override | ✓ | ✓ | ✓ | ✓ |
| Token cache replacement | ✓ | ✓ | ✓ | ✓ |
| Invalid-token retry for JSON APIs | ✓ | ✓ | ✓ | ✓ |
| JSON/Form/Bytes/Multipart | ✓ | ✓ | ✓ | ✓ |
| Generic raw OpenAPI call | ✓ | ✓ | ✓ | ✓ |
| Event signature verification | ✓ | ✓ | ✓ | ✓ |
| Event AES decryption | ✓ | ✓ | ✓ | ✓ |
| One-click app registration | ✓ | ✓ | ✓ | ✓ |
| App preset / add-ons / domain switch | ✓ | ✓ | ✓ | ✓ |
| Full generated service coverage | ✓ | ✓ | ✓ | In progress |
| WebSocket long connection | ✓ | ✓ | ✓ | Planned |
| Client assertion OAuth | ✓ | varies | ✓ | Planned |

The generic request layer provides immediate access to APIs that do not yet have generated Rust types.
