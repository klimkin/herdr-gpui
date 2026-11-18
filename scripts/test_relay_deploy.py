"""The relay's DigitalOcean user data, rendered the way create-droplet.sh
renders it, picks the domain it was given, or IP.sslip.io without one, keeps
the relay to computers with its listen token unless it is open, and runs
only a relay image whose signature it verified."""

import json
import os
import re
import shutil
import subprocess
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLOUD_INIT = os.path.join(ROOT, "deploy", "relay", "cloud-init.yml")
CREATE_DROPLET = os.path.join(ROOT, "deploy", "relay", "create-droplet.sh")
LISTEN_TOKEN = "q2Xv-Jk_8sQb3N7mZp0aW4yRt6uE1oLcHd9fGi5BjKw"
TOKEN = re.compile(r"[A-Za-z0-9_-]{43}")


def rendered(domain: str, listen_token: str = "open") -> str:
    """cloud-init.yml after create-droplet.sh's substitution."""
    with open(CLOUD_INIT, encoding="utf-8") as file:
        text = file.read()
    return (
        text.replace("__COMPOSE_B64__", "Y29tcG9zZQ==")
        .replace("__CADDYFILE_B64__", "Y2FkZHk=")
        .replace("__RELAY_DOMAIN__", domain)
        .replace("__RELAY_LISTEN_TOKEN__", listen_token)
    )


def first_secrets(text: str) -> tuple[str, str]:
    """Runs start.sh's first-boot secrets block for relay.example.org in an
    empty directory, and returns the .env and the login file it writes."""
    block = re.search(r"\n(\s*)if \[ ! -f \.env \]; then\n.*?\n\1fi\n", text, re.S)
    assert block, "start.sh's secrets block is missing"
    script = "\n".join(line.strip() for line in block.group(0).splitlines())
    with tempfile.TemporaryDirectory() as directory:
        login = os.path.join(directory, "login.txt")
        script = 'domain=relay.example.org\n' + script.replace("/root/sheprd-relay-login.txt", login)
        subprocess.run(["sh", "-c", script], cwd=directory, check=True)
        with open(os.path.join(directory, ".env"), encoding="utf-8") as env, open(login, encoding="utf-8") as file:
            return env.read(), file.read()


def script_function(name: str) -> str:
    """One of create-droplet.sh's shell functions, as its source."""
    with open(CREATE_DROPLET, encoding="utf-8") as file:
        block = re.search(rf"\n{name}\(\) \{{\n.*?\n\}}\n", file.read(), re.S)
    assert block, f"create-droplet.sh's {name} is missing"
    return block.group(0)


def chosen_domain(text: str) -> str:
    """Runs start.sh's domain choice, with the metadata service answering
    203.0.113.7, and returns the domain it settles on."""
    block = re.search(r'\n(\s*)domain="[^"\n]*"\n.*?\n\1fi\n', text, re.S)
    assert block, "start.sh's domain block is missing"
    script = "\n".join(line.strip() for line in block.group(0).splitlines())
    shell = 'curl() { echo 203.0.113.7; }\n' + script + '\nprintf "%s" "$domain"\n'
    with tempfile.TemporaryDirectory() as directory:
        path = os.path.join(directory, "domain.sh")
        with open(path, "w", encoding="utf-8") as file:
            file.write(shell)
        return subprocess.run(
            ["sh", path], check=True, capture_output=True, text=True
        ).stdout


def bans_mode(text: str) -> tuple[int, int]:
    """Runs start.sh's ban list setup under the umask start.sh has by then,
    and returns the directory's and the list's permissions."""
    block = re.search(r"\n\s*\(umask 022; mkdir -p bans.*?chmod 644 bans/list\n", text, re.S)
    assert block, "start.sh's ban list block is missing"
    script = "umask 077\n" + "\n".join(line.strip() for line in block.group(0).splitlines())
    with tempfile.TemporaryDirectory() as directory:
        subprocess.run(["sh", "-c", script], cwd=directory, check=True)
        bans = os.path.join(directory, "bans")
        return os.stat(bans).st_mode & 0o777, os.stat(os.path.join(bans, "list")).st_mode & 0o777


IDENTITY = "https://github.com/sheprd-labs/sheprd/.github/workflows/relay-image.yml@refs/heads/main"
ISSUER = "https://token.actions.githubusercontent.com"
DIGEST = "sha256:" + "ab" * 32


def signatures(*digests: str) -> str:
    """A fake cosign that logs its arguments and verifies DIGESTS."""
    payload = json.dumps([{"critical": {"image": {"docker-manifest-digest": d}}} for d in digests])
    return f"echo \"$@\" >&2\nprintf '%s' '{payload}'"


def verified_image(text: str, cosign: str, relay_image: str = "") -> subprocess.CompletedProcess:
    """Runs start.sh's image check with COSIGN as the cosign script's body;
    stdout is the image compose would run."""
    block = re.search(r"\n(\s*)verified_image\(\) \{\n.*?\n\1\}\n", text, re.S)
    assert block, "start.sh's image check is missing"
    script = "\n".join(line.strip() for line in block.group(0).splitlines())
    with tempfile.TemporaryDirectory() as directory:
        fake = os.path.join(directory, "cosign")
        with open(fake, "w", encoding="utf-8") as file:
            file.write("#!/bin/sh\n" + cosign + "\n")
        os.chmod(fake, 0o755)
        shell = f'cosign="{fake}"\n{script}\nimage=$(verified_image) || exit 1\nprintf "%s" "$image"\n'
        environment = {"PATH": os.environ["PATH"], "RELAY_IMAGE": relay_image}
        return subprocess.run(["sh", "-c", shell], capture_output=True, text=True, env=environment)


@unittest.skipUnless(shutil.which("jq"), "jq is not installed")
class ImageTest(unittest.TestCase):
    def test_the_tag_runs_as_the_digest_its_signature_covers(self) -> None:
        result = verified_image(rendered(""), signatures(DIGEST))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, f"ghcr.io/sheprd-labs/sheprd-relay@{DIGEST}")
        self.assertIn(
            f"verify ghcr.io/sheprd-labs/sheprd-relay:latest --certificate-identity {IDENTITY} "
            f"--certificate-oidc-issuer {ISSUER}",
            result.stderr,
        )

    def test_a_bad_signature_runs_nothing(self) -> None:
        result = verified_image(rendered(""), "exit 1")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")
        self.assertIn("the running relay is unchanged", result.stderr)

    def test_signatures_naming_two_digests_run_nothing(self) -> None:
        result = verified_image(rendered(""), signatures(DIGEST, "sha256:" + "cd" * 32))
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")

    def test_an_operators_digest_is_used_as_given(self) -> None:
        pinned = f"registry.example.org:5000/relay@{DIGEST}"
        result = verified_image(rendered(""), "exit 1", pinned)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, pinned)

    def test_an_operators_tag_is_verified(self) -> None:
        tagged = "registry.example.org:5000/relay:v2"
        result = verified_image(rendered(""), signatures(DIGEST), tagged)
        self.assertEqual(result.stdout, f"registry.example.org:5000/relay@{DIGEST}")
        self.assertIn(f"verify {tagged} --certificate-identity {IDENTITY}", result.stderr)


class StartTest(unittest.TestCase):
    def test_compose_runs_the_verified_image_without_pulling_tags(self) -> None:
        text = rendered("")
        self.assertNotIn("--pull always", text)
        self.assertIn('RELAY_IMAGE="$image" docker compose --env-file compose.env up -d', text)
        self.assertLess(text.index("image=$(verified_image) || exit 1"), text.index("docker compose"))

    def test_cosign_is_checked_against_a_pinned_hash(self) -> None:
        hashes = re.findall(r"(amd64|arm64)\) cosign_sha256=([0-9a-f]{64}) ;;", rendered(""))
        self.assertEqual({arch for arch, _ in hashes}, {"amd64", "arm64"})


class ListenTokenTest(unittest.TestCase):
    def test_a_filled_token_keeps_the_relay_to_its_computers(self) -> None:
        env, login = first_secrets(rendered("", LISTEN_TOKEN))
        self.assertIn(f"\nRELAY_LISTEN_TOKEN={LISTEN_TOKEN}\n", env)
        self.assertIn(f"listen token: {LISTEN_TOKEN}\n", login)
        self.assertIn(
            f"sheprd://relay?endpoint=https%3A%2F%2Frelay.example.org&token={LISTEN_TOKEN}\n", login
        )

    def test_open_or_unfilled_leaves_the_relay_open(self) -> None:
        with open(CLOUD_INIT, encoding="utf-8") as file:
            unfilled = file.read()
        for text in (rendered("", "open"), unfilled):
            env, login = first_secrets(text)
            self.assertIn("RELAY_DASHBOARD_TOKEN=", env)
            self.assertNotIn("LISTEN_TOKEN", env)
            self.assertNotIn("listen token", login)

    def test_the_token_reaches_the_relay_container(self) -> None:
        with open(os.path.join(ROOT, "deploy", "relay", "compose.yml"), encoding="utf-8") as file:
            self.assertIn("env_file: .env", file.read())

    def test_create_droplet_makes_a_token_unless_open(self) -> None:
        with open(CREATE_DROPLET, encoding="utf-8") as file:
            script = file.read()
        self.assertIn('if [ "${RELAY_OPEN:-}" != 1 ]; then', script)
        tokens = subprocess.run(
            ["bash", "-c", script_function("listen_token") + "listen_token; echo; listen_token"],
            check=True, capture_output=True, text=True,
        ).stdout.split()
        self.assertEqual(len(tokens), 2)
        self.assertNotEqual(tokens[0], tokens[1])
        for token in tokens:
            self.assertRegex(token, TOKEN.pattern + "$")


class BansTest(unittest.TestCase):
    def test_the_ban_list_is_readable_by_the_relay(self) -> None:
        self.assertEqual(bans_mode(rendered("")), (0o755, 0o644))


def service_block(path: str, indent: int) -> list[str]:
    """The relay service's lines in an App Platform spec, without their
    indentation, from its `- name: relay` to the end of the file."""
    with open(os.path.join(ROOT, path), encoding="utf-8") as file:
        lines = file.read().splitlines()
    start = next(index for index, line in enumerate(lines) if line.strip() == "- name: relay")
    return [line[indent:] for line in lines[start:] if line.strip()]


class AppPlatformTest(unittest.TestCase):
    def test_the_deploy_button_template_matches_the_doctl_spec(self) -> None:
        self.assertEqual(
            service_block(".do/deploy.template.yaml", 4),
            service_block("deploy/relay/app.yaml", 2),
        )

    def test_the_image_is_the_published_one(self) -> None:
        for path in (".do/deploy.template.yaml", "deploy/relay/app.yaml"):
            with self.subTest(path=path):
                block = "\n".join(service_block(path, 0))
                self.assertIn("registry: ghcr.io", block)
                self.assertIn("repository: sheprd-labs/sheprd-relay", block)


class DomainTest(unittest.TestCase):
    def test_a_given_domain_is_kept(self) -> None:
        self.assertEqual(
            chosen_domain(rendered("nyc3.relay.example.org")), "nyc3.relay.example.org"
        )

    def test_no_domain_falls_back_to_sslip(self) -> None:
        self.assertEqual(chosen_domain(rendered("")), "203-0-113-7.sslip.io")

    def test_an_unfilled_placeholder_falls_back_to_sslip(self) -> None:
        with open(CLOUD_INIT, encoding="utf-8") as file:
            self.assertEqual(chosen_domain(file.read()), "203-0-113-7.sslip.io")


PAGE_SCRIPT = os.path.join(ROOT, "website", "site", "relay", "deploy.js")


def page_rendered(domain: str, listen_token: str) -> str:
    """The user data sheprd.org/relay sends, from deploy.js's render."""
    sources = [os.path.join(ROOT, "deploy", "relay", name) for name in ("cloud-init.yml", "compose.yml", "Caddyfile")]
    script = (
        "const fs = require('fs');"
        f"const page = require({json.dumps(PAGE_SCRIPT)});"
        "const [a, b, c, d, e] = process.argv.slice(1);"
        "process.stdout.write(page.render(...[a, b, c].map(p => fs.readFileSync(p, 'utf8')), d, e));"
    )
    return subprocess.run(
        ["node", "-e", script, *sources, domain, listen_token], check=True, capture_output=True, text=True
    ).stdout


def script_rendered(domain: str, listen_token: str) -> str:
    """The user data create-droplet.sh sends, from its own commands."""
    here = os.path.join(ROOT, "deploy", "relay")
    with open(CREATE_DROPLET, encoding="utf-8") as file:
        source = file.read()
    start = source.index("  rendered=$(sed ")
    end = source.index('> "$data"\n', start)
    shell = (
        'here=$1 domain=$2 token=$3\n'
        'compose=$(base64 < "$here/compose.yml" | tr -d \'\\n\')\n'
        'caddyfile=$(base64 < "$here/Caddyfile" | tr -d \'\\n\')\n'
        + source[start:end]
    )
    return subprocess.run(
        ["bash", "-c", shell, "render", here, domain, listen_token], check=True, capture_output=True, text=True
    ).stdout


@unittest.skipUnless(shutil.which("node"), "node is not installed")
class DeployPageTest(unittest.TestCase):
    def test_the_page_sends_what_create_droplet_sends(self) -> None:
        for domain in ("", "nyc3.relay.example.org"):
            for listen_token in ("open", LISTEN_TOKEN):
                with self.subTest(domain=domain, listen_token=listen_token):
                    page = page_rendered(domain, listen_token)
                    self.assertEqual(page, script_rendered(domain, listen_token))
                    self.assertNotIn("__RELAY_LISTEN_TOKEN__", page)

    def test_the_page_makes_tokens_and_links_as_the_script_does(self) -> None:
        address = "https://203-0-113-7.sslip.io"
        script = (
            f"const page = require({json.dumps(PAGE_SCRIPT)});"
            "console.log(page.listenToken());"
            "console.log(page.listenToken());"
            f"console.log(page.addLink({json.dumps(address)}, {json.dumps(LISTEN_TOKEN)}));"
        )
        first, second, link = subprocess.run(
            ["node", "-e", script], check=True, capture_output=True, text=True
        ).stdout.splitlines()
        self.assertRegex(first, TOKEN.pattern + "$")
        self.assertNotEqual(first, second)
        encoded = subprocess.run(
            ["bash", "-c", script_function("encoded") + 'encoded "$1"', "encode", address],
            check=True, capture_output=True, text=True,
        ).stdout
        self.assertEqual(link, f"sheprd://relay?endpoint={encoded}&token={LISTEN_TOKEN}")
        self.assertEqual(encoded, "https%3A%2F%2F203-0-113-7.sslip.io")

    def test_the_page_addresses_relays_as_the_script_prints_them(self) -> None:
        script = (
            f"const page = require({json.dumps(PAGE_SCRIPT)});"
            "console.log(page.relayAddress('203.0.113.7', 'nyc3', ''));"
            "console.log(page.relayAddress('203.0.113.7', 'nyc3', 'relay.example.org'));"
            "for (const v of [' Relay.Example.org. ', '', 'bad name', 'a.b;rm', '-x.org'])"
            " console.log(JSON.stringify(page.normalizedSuffix(v)));"
        )
        lines = subprocess.run(["node", "-e", script], check=True, capture_output=True, text=True).stdout.splitlines()
        self.assertEqual(lines, [
            "https://203-0-113-7.sslip.io",
            "https://nyc3.relay.example.org",
            '"relay.example.org"', '""', "null", "null", "null",
        ])


class DeployPageSourceTest(unittest.TestCase):
    def test_the_page_reads_the_files_create_droplet_reads_from_main(self) -> None:
        with open(PAGE_SCRIPT, encoding="utf-8") as file:
            page = file.read()
        self.assertIn("https://raw.githubusercontent.com/sheprd-labs/sheprd/main/deploy/relay/", page)
        for name in ("cloud-init.yml", "compose.yml", "Caddyfile"):
            with self.subTest(file=name):
                self.assertIn(f"'{name}'", page)
                self.assertTrue(os.path.isfile(os.path.join(ROOT, "deploy", "relay", name)))
        with open(os.path.join(ROOT, "deploy", "relay", "create-droplet.sh"), encoding="utf-8") as file:
            script = file.read()
        for setting in ("--image ubuntu-24-04-x64", "--tag-name sheprd-relay", "s-1vcpu-1gb"):
            with self.subTest(setting=setting):
                self.assertIn(setting, script)
        self.assertIn("const IMAGE = 'ubuntu-24-04-x64';", page)
        self.assertIn("const TAG = 'sheprd-relay';", page)

if __name__ == "__main__":
    unittest.main()
