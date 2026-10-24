CREATE TABLE "devbox_blocks" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid NOT NULL,
	"finding_uuid" uuid NOT NULL,
	"server_uuid" uuid,
	"rule" varchar(64),
	"lifted" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "devboxes" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"game_server_uuid" uuid NOT NULL,
	"user_uuid" uuid,
	"server_uuid" uuid,
	"node_uuid" uuid,
	"mount_source" varchar(4096),
	"slug" varchar(24) NOT NULL,
	"token_hash" varchar(64),
	"code_secret" text,
	"stopping" timestamp,
	"boot_status" varchar(32),
	"boot_detail" varchar(255),
	"host_key" varchar(100),
	"version" varchar(40),
	"connections" integer DEFAULT 0 NOT NULL,
	"shared_rights" jsonb,
	"people" uuid[],
	"started" timestamp,
	"booted" timestamp,
	"last_seen" timestamp,
	"last_used" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE UNIQUE INDEX "devbox_blocks_user_uuid_finding_uuid_idx" ON "devbox_blocks" ("user_uuid","finding_uuid");
CREATE INDEX "devbox_blocks_finding_uuid_idx" ON "devbox_blocks" ("finding_uuid");
CREATE UNIQUE INDEX "devboxes_slug_idx" ON "devboxes" ("slug");
CREATE UNIQUE INDEX "devboxes_server_uuid_idx" ON "devboxes" ("server_uuid");
CREATE UNIQUE INDEX "devboxes_token_hash_idx" ON "devboxes" ("token_hash");
CREATE UNIQUE INDEX "devboxes_game_server_uuid_user_uuid_idx" ON "devboxes" ("game_server_uuid","user_uuid") WHERE user_uuid IS NOT NULL;
CREATE UNIQUE INDEX "devboxes_game_server_uuid_shared_idx" ON "devboxes" ("game_server_uuid") WHERE user_uuid IS NULL;
CREATE INDEX "devboxes_user_uuid_idx" ON "devboxes" ("user_uuid");
ALTER TABLE "devbox_blocks" ADD CONSTRAINT "devbox_blocks_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;
ALTER TABLE "devboxes" ADD CONSTRAINT "devboxes_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE SET NULL;