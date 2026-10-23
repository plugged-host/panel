CREATE TABLE "ad_banners" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"advertiser" varchar(1020) NOT NULL,
	"link" varchar(1024) NOT NULL,
	"placement" varchar(16) NOT NULL,
	"image" varchar(255),
	"width" integer DEFAULT 0 NOT NULL,
	"height" integer DEFAULT 0 NOT NULL,
	"starts" timestamp,
	"ends" timestamp,
	"active" boolean DEFAULT true NOT NULL,
	"clicks" bigint DEFAULT 0 NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

CREATE TABLE "server_waiting_rooms" (
	"server_uuid" uuid,
	"user_uuid" uuid,
	"action" varchar(16) NOT NULL,
	"opened" timestamp DEFAULT now() NOT NULL,
	"dispatched" boolean DEFAULT false NOT NULL,
	"released" timestamp,
	"blocked" boolean,
	CONSTRAINT "server_waiting_rooms_server_uuid_user_uuid_pk" PRIMARY KEY("server_uuid","user_uuid")
);

CREATE INDEX "ad_banners_created_idx" ON "ad_banners" ("created");
ALTER TABLE "server_waiting_rooms" ADD CONSTRAINT "server_waiting_rooms_server_uuid_servers_uuid_fkey" FOREIGN KEY ("server_uuid") REFERENCES "servers"("uuid") ON DELETE CASCADE;
ALTER TABLE "server_waiting_rooms" ADD CONSTRAINT "server_waiting_rooms_user_uuid_users_uuid_fkey" FOREIGN KEY ("user_uuid") REFERENCES "users"("uuid") ON DELETE CASCADE;