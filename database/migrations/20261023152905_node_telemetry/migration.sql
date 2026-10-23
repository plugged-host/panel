CREATE TABLE "node_telemetry_samples" (
	"node_uuid" uuid,
	"reachable" boolean NOT NULL,
	"refused" boolean DEFAULT false NOT NULL,
	"response_time_ms" integer,
	"cpu_percent" real,
	"memory_used" bigint,
	"memory_total" bigint,
	"disk_used" bigint,
	"disk_total" bigint,
	"load1" real,
	"servers_running" integer,
	"sampled" timestamp DEFAULT now(),
	CONSTRAINT "node_telemetry_samples_pk" PRIMARY KEY("node_uuid","sampled")
);

CREATE TABLE "node_telemetry" (
	"node_uuid" uuid PRIMARY KEY,
	"reachable" boolean NOT NULL,
	"refused" boolean DEFAULT false NOT NULL,
	"response_time_ms" integer,
	"cpu_percent" real,
	"cpu_threads" integer,
	"memory_used" bigint,
	"memory_total" bigint,
	"disk_used" bigint,
	"disk_total" bigint,
	"load1" real,
	"servers_running" integer,
	"pressure" real,
	"consecutive_failures" integer DEFAULT 0 NOT NULL,
	"consecutive_outages" integer DEFAULT 0 NOT NULL,
	"error" text,
	"last_reachable" timestamp,
	"sampled" timestamp DEFAULT now() NOT NULL
);

ALTER TABLE "locations" ADD COLUMN "tier" varchar(32);
ALTER TABLE "locations" ADD COLUMN "sort" integer DEFAULT 0 NOT NULL;
ALTER TABLE "locations" ADD COLUMN "public" boolean DEFAULT true NOT NULL;
ALTER TABLE "nodes" ADD COLUMN "role" varchar(32) DEFAULT 'game' NOT NULL;
ALTER TABLE "nodes" ADD COLUMN "tags" varchar(64)[] DEFAULT '{}'::varchar(64)[] NOT NULL;
ALTER TABLE "nodes" ADD COLUMN "srv_target" varchar(255);
ALTER TABLE "nodes" ADD COLUMN "deployment_paused" boolean DEFAULT false NOT NULL;
CREATE INDEX "node_telemetry_samples_sampled_idx" ON "node_telemetry_samples" ("sampled");
ALTER TABLE "node_telemetry_samples" ADD CONSTRAINT "node_telemetry_samples_node_uuid_nodes_uuid_fkey" FOREIGN KEY ("node_uuid") REFERENCES "nodes"("uuid") ON DELETE CASCADE;
ALTER TABLE "node_telemetry" ADD CONSTRAINT "node_telemetry_node_uuid_nodes_uuid_fkey" FOREIGN KEY ("node_uuid") REFERENCES "nodes"("uuid") ON DELETE CASCADE;