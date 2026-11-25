CREATE TABLE "discord_message_counts" (
	"guild_id" varchar(32),
	"discord_id" varchar(32),
	"day" date,
	"messages" integer DEFAULT 0 NOT NULL,
	CONSTRAINT "discord_message_counts_pk" PRIMARY KEY("guild_id","discord_id","day")
);

CREATE INDEX "discord_message_counts_day_idx" ON "discord_message_counts" ("day");