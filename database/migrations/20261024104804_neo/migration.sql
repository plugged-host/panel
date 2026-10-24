CREATE TABLE "neo_conversations" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid NOT NULL,
	"server_uuid" uuid,
	"title" varchar(480),
	"status" varchar(24) DEFAULT 'idle' NOT NULL,
	"step_count" smallint DEFAULT 0 NOT NULL,
	"tokens_used" bigint DEFAULT 0 NOT NULL,
	"last_provider" varchar(64),
	"last_error" varchar(255),
	"last_activity" timestamp DEFAULT now() NOT NULL,
	"archived" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "neo_images" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"message_uuid" uuid NOT NULL,
	"data" bytea NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "neo_messages" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"conversation_uuid" uuid NOT NULL,
	"seq" integer NOT NULL,
	"role" varchar(12) NOT NULL,
	"content" text,
	"tool_calls" jsonb,
	"tool_call_id" varchar(64),
	"tool_name" varchar(64),
	"status" varchar(24),
	"meta" jsonb,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "neo_usage_events" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid,
	"discord_id" varchar(20),
	"service" varchar(32) NOT NULL,
	"conversation" varchar(64),
	"provider_slot" smallint,
	"provider_label" varchar(64),
	"driver" varchar(16),
	"model" varchar(128),
	"prompt_tokens" integer DEFAULT 0 NOT NULL,
	"completion_tokens" integer DEFAULT 0 NOT NULL,
	"charged_tokens" bigint DEFAULT 0 NOT NULL,
	"finish_reason" varchar(24),
	"status" varchar(16) NOT NULL,
	"attempts" jsonb,
	"duration_ms" integer DEFAULT 0 NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "neo_usage_windows" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"user_uuid" uuid,
	"discord_id" varchar(20),
	"kind" varchar(8) NOT NULL,
	"started" timestamp DEFAULT now() NOT NULL,
	"resets" timestamp DEFAULT now() NOT NULL,
	"tokens_used" bigint DEFAULT 0 NOT NULL
);

CREATE TABLE "user_neo_settings" (
	"user_uuid" uuid PRIMARY KEY,
	"enabled" boolean DEFAULT true NOT NULL,
	"auto_approve" boolean DEFAULT false NOT NULL,
	"model" varchar(16),
	"acknowledged" timestamp,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE INDEX "neo_conversations_user_uuid_server_uuid_archived_idx" ON "neo_conversations" ("user_uuid","server_uuid","archived");
CREATE INDEX "neo_conversations_last_activity_idx" ON "neo_conversations" ("last_activity");
CREATE INDEX "neo_images_message_uuid_idx" ON "neo_images" ("message_uuid");
CREATE UNIQUE INDEX "neo_messages_conversation_uuid_seq_idx" ON "neo_messages" ("conversation_uuid","seq");
CREATE INDEX "neo_usage_events_user_uuid_created_idx" ON "neo_usage_events" ("user_uuid","created");
CREATE INDEX "neo_usage_events_discord_id_idx" ON "neo_usage_events" ("discord_id");
CREATE INDEX "neo_usage_events_created_idx" ON "neo_usage_events" ("created");
CREATE UNIQUE INDEX "neo_usage_windows_user_uuid_kind_idx" ON "neo_usage_windows" ("user_uuid","kind") WHERE user_uuid IS NOT NULL;
CREATE UNIQUE INDEX "neo_usage_windows_discord_id_kind_idx" ON "neo_usage_windows" ("discord_id","kind") WHERE user_uuid IS NULL;
CREATE INDEX "neo_usage_windows_resets_idx" ON "neo_usage_windows" ("resets");
ALTER TABLE "neo_conversations" ADD CONSTRAINT "neo_conversations_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;
ALTER TABLE "neo_conversations" ADD CONSTRAINT "neo_conversations_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE CASCADE;
ALTER TABLE "neo_images" ADD CONSTRAINT "neo_images_message_uuid_neo_messages_uuid_fkey" FOREIGN KEY ("message_uuid") REFERENCES "neo_messages"("uuid") ON DELETE CASCADE;
ALTER TABLE "neo_messages" ADD CONSTRAINT "neo_messages_conversation_uuid_neo_conversations_uuid_fkey" FOREIGN KEY ("conversation_uuid") REFERENCES "neo_conversations"("uuid") ON DELETE CASCADE;
ALTER TABLE "neo_usage_events" ADD CONSTRAINT "neo_usage_events_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE SET NULL;
ALTER TABLE "neo_usage_windows" ADD CONSTRAINT "neo_usage_windows_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;
ALTER TABLE "user_neo_settings" ADD CONSTRAINT "user_neo_settings_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;