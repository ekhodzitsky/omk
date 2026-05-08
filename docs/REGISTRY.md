# Marketplace Registry Format

OMK supports external marketplace registries. You can host your own registry as a static JSON file.

## Registry JSON Format

```json
{
  "name": "My Custom Registry",
  "url": "https://example.com/omk-registry.json",
  "skills": [
    {
      "name": "rust-expert",
      "description": "Advanced Rust patterns and async best practices",
      "author": "@yourhandle",
      "url": "https://github.com/yourhandle/omk-skill-rust",
      "tags": ["rust", "systems"]
    }
  ]
}
```

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Registry display name |
| `url` | string | yes | Canonical URL of the registry file |
| `skills` | array | yes | List of skills |

### Skill Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Unique skill identifier |
| `description` | string | yes | Short description |
| `author` | string | yes | Author handle or name |
| `url` | string | yes | Git repository URL for installation |
| `tags` | string[] | no | Searchable tags |

## Hosting Options

### Static JSON (GitHub Pages, S3, etc.)

Host the JSON file anywhere accessible via HTTP(S):

```bash
omk marketplace add-registry https://example.com/omk-registry.json
```

### Local File

Use a local JSON file for private or air-gapped registries:

```bash
omk marketplace add-registry /path/to/registry.json
```

## Managing Registries

```bash
# Add a registry
omk marketplace add-registry https://example.com/registry.json

# List configured registries
omk marketplace list-registries

# Remove a registry
omk marketplace remove-registry https://example.com/registry.json

# Use a specific registry for one command
omk marketplace list --registry https://example.com/registry.json
omk marketplace install my-skill --registry https://example.com/registry.json
```

## Example Registry

See the built-in curated list in `src/cli/marketplace.rs` for reference.
