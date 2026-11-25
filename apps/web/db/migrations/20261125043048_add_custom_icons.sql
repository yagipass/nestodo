CREATE TABLE `custom_icons` (
	`id` text PRIMARY KEY NOT NULL,
	`user_id` text NOT NULL,
	`name` text NOT NULL,
	`created_at` text DEFAULT (strftime('%Y-%m-%d %H:%M:%f', 'now')) NOT NULL
);
--> statement-breakpoint
CREATE INDEX `custom_icons_user_id_created_at_idx` ON `custom_icons` (`user_id`,`created_at`);