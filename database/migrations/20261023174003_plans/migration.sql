CREATE TABLE "plans" (
	"uuid" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
	"name" varchar(1020) NOT NULL,
	"description" text,
	"cpu" integer NOT NULL,
	"memory" bigint NOT NULL,
	"disk" bigint NOT NULL,
	"sort" integer DEFAULT 0 NOT NULL,
	"public" boolean DEFAULT true NOT NULL,
	"created" timestamp DEFAULT now() NOT NULL
);

ALTER TABLE "nodes" ADD COLUMN "deployment_paused_reason" varchar(32);
CREATE UNIQUE INDEX "plans_name_idx" ON "plans" ("name");