import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import assert from 'node:assert/strict';
import { test } from 'node:test';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(__dirname, '..');
const secretsPath = join(repoRoot, 'src', 'scripts', 'secrets.js');
const source = readFileSync(secretsPath, 'utf8');

test('SECRET_KEYS exposes GITHUB_TOKEN pointing at "github_token"', () => {
    assert.match(
        source,
        /GITHUB_TOKEN:\s*['"]github_token['"]/,
        'SECRET_KEYS.GITHUB_TOKEN must equal "github_token" so backend SecretKeys::GITHUB_TOKEN matches',
    );
});
