plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

val baseUrl =
    providers.gradleProperty("nestodo.baseUrl").orNull
        ?: error(
            "Set nestodo.baseUrl to your Nestodo server in ~/.gradle/gradle.properties, " +
                "or pass -Pnestodo.baseUrl=https://nestodo.example.com"
        )

android {
    namespace = "io.github.yagipass.nestodo"
    compileSdk = 37

    defaultConfig {
        applicationId = "io.github.yagipass.nestodo"
        minSdk = 37
        targetSdk = 37
        versionCode = 1
        versionName = "0.9.1"
        buildConfigField("String", "BASE_URL", "\"$baseUrl\"")
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    androidResources { localeFilters += "en" }

    buildFeatures {
        buildConfig = true
        compose = true
    }
}

dependencies {
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.browser)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.bouncycastle)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.okhttp)
    implementation(libs.okhttp.coroutines)
    debugImplementation(libs.androidx.compose.ui.tooling)
    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.okhttp.mockwebserver)
}
