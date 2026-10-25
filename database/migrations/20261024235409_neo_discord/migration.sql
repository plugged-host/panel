CREATE TABLE "neo_discord_messages" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"conversation" varchar(48) NOT NULL,
	"turn_uuid" uuid,
	"position" smallint DEFAULT 0 NOT NULL,
	"discord_message_id" varchar(20),
	"channel_id" varchar(20) NOT NULL,
	"guild_id" varchar(20),
	"author_id" varchar(20) NOT NULL,
	"author_name" varchar(400),
	"role" varchar(12) NOT NULL,
	"content" text,
	"tool_calls" jsonb,
	"tool_call_id" varchar(64),
	"tool_name" varchar(64),
	"status" varchar(12) NOT NULL,
	"meta" jsonb,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE INDEX "neo_discord_messages_conversation_created_idx" ON "neo_discord_messages" ("conversation","created");
CREATE INDEX "neo_discord_messages_turn_uuid_idx" ON "neo_discord_messages" ("turn_uuid");
CREATE INDEX "neo_discord_messages_created_idx" ON "neo_discord_messages" ("created");