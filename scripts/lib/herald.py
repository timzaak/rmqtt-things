"""
Herald auth service permission initialization.

Inserts rmqtt-things permissions (product/device/cert x read/write)
into Herald's PostgreSQL database after the service has started.
"""

from . import docker

_REALM_ID = "default"
_CLIENT_ID = "rmqtt-things-admin"
_ROLE_NAME = "things-admin"
_TEST_USER_EMAIL = "admin@rmqtt-things.local"

_PERMISSIONS = [
    ("product", "read", "View products and validation templates"),
    ("product", "write", "Create or edit products/templates and upload files"),
    ("device", "read", "View device status, properties, events, and commands"),
    ("device", "write", "Issue and delete property commands"),
    ("cert", "read", "View certificates and OTA versions"),
    ("cert", "write", "Issue/revoke certificates and manage OTA versions"),
]

_INIT_SQL = """\
DO $$
DECLARE
    v_role_id uuid;
    v_user_id uuid;
    v_password_hash text;
BEGIN
    -- 1. Create realm
    INSERT INTO realm (id, name) VALUES ('{realm}', 'Default Realm')
        ON CONFLICT (id) DO NOTHING;

    -- 2. Create client app
    INSERT INTO client_app (id, realm_id, client_id, name)
    VALUES (uuidv7(), '{realm}', '{client}', 'RMQTT Things Admin')
        ON CONFLICT (realm_id, client_id) DO NOTHING;

    -- 3. Create role
    INSERT INTO roles (id, name, description, realm_id, client_id, is_builtin)
    VALUES (uuidv7(), '{role}', 'RMQTT Things Administrator', '{realm}', '{client}', true)
        ON CONFLICT (name, realm_id, client_id) DO NOTHING;

    SELECT id INTO v_role_id FROM roles
        WHERE name = '{role}' AND realm_id = '{realm}' AND client_id = '{client}';

    -- 4. Upsert permission definitions required by rmqtt-things
    INSERT INTO permissions (id, name, description, realm_id, resource, action, is_builtin)
    VALUES
        {permission_defs}
    ON CONFLICT (name, realm_id) DO UPDATE SET
        description = EXCLUDED.description,
        resource = EXCLUDED.resource,
        action = EXCLUDED.action,
        is_builtin = EXCLUDED.is_builtin,
        updated_at = CURRENT_TIMESTAMP;

    -- 5. Upsert role policies (resource x action) used by Herald permission checks
    DELETE FROM role_policies WHERE role_id = v_role_id;
    INSERT INTO role_policies (id, role_id, realm_id, resource, action, effect) VALUES
        {policies};

    -- 6. Assign permission definitions to the role for Herald management APIs/UI
    INSERT INTO role_permissions (id, role_id, permission_id)
    SELECT uuidv7(), v_role_id, p.id
    FROM permissions p
    WHERE p.realm_id = '{realm}'
      AND (p.resource, p.action) IN ({permission_pairs})
    ON CONFLICT (role_id, permission_id) DO NOTHING;

    -- 7. Reuse Herald admin user password hash
    SELECT password INTO v_password_hash
        FROM account WHERE realm_id = 'admin' LIMIT 1;

    -- 8. Create test user
    INSERT INTO account (id, realm_id, email, password, status)
    VALUES (uuidv7(), '{realm}', '{email}', v_password_hash, 1)
        ON CONFLICT (realm_id, email) DO NOTHING;

    SELECT id INTO v_user_id FROM account
        WHERE email = '{email}' AND realm_id = '{realm}';

    INSERT INTO profile (id, realm_id, nickname)
    VALUES (v_user_id, '{realm}', 'Test Admin')
        ON CONFLICT (id, realm_id) DO NOTHING;

    -- 9. Assign role to user
    INSERT INTO user_roles (id, user_id, role_id, realm_id, client_id)
    VALUES (uuidv7(), v_user_id, v_role_id, '{realm}', '{client}')
        ON CONFLICT (user_id, role_id, realm_id) DO NOTHING;
END $$;
"""


def init_permissions(pg_container: str, pg_user: str, herald_db: str) -> bool:
    """Insert rmqtt-things permissions into Herald's database.

    Must be called after Herald has started and auto-migrated its tables.

    Args:
        pg_container: PostgreSQL container name hosting Herald's DB.
        pg_user: PostgreSQL user for psql commands.
        herald_db: Herald database name (e.g. "herald_test").

    Returns:
        True on success, False on failure.
    """
    permission_defs = ",\n        ".join(
        f"(uuidv7(), '{res}:{act}', '{desc}', '{_REALM_ID}', '{res}', '{act}', true)"
        for res, act, desc in _PERMISSIONS
    )
    policy_values = ",\n        ".join(
        f"(uuidv7(), v_role_id, '{_REALM_ID}', '{res}', '{act}', true)"
        for res, act, _ in _PERMISSIONS
    )
    permission_pairs = ", ".join(
        f"('{res}', '{act}')"
        for res, act, _ in _PERMISSIONS
    )

    sql = _INIT_SQL.format(
        realm=_REALM_ID,
        client=_CLIENT_ID,
        role=_ROLE_NAME,
        email=_TEST_USER_EMAIL,
        permission_defs=permission_defs,
        policies=policy_values,
        permission_pairs=permission_pairs,
    )

    code, out = docker.exec_check(
        pg_container,
        ["psql", "-U", pg_user, "-d", herald_db, "-v", "ON_ERROR_STOP=1", "-c", sql],
    )
    if code == 0:
        print("Herald permissions initialized")
        return True
    print(f"ERROR: Herald permissions init failed: {out}")
    return False
