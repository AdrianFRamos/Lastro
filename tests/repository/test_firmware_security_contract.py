from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SIGNER = ROOT / 'firmware/station/components/lastro_station/signer.c'
KCONFIG = ROOT / 'firmware/station/components/lastro_station/Kconfig'


def test_station_signer_never_programs_or_changes_efuse_state():
    """The runtime signer may consume a provisioned opaque key but must never provision hardware."""
    text = SIGNER.read_text(encoding='utf-8')
    forbidden = (
        'esp_efuse_write_key(',
        'esp_efuse_write_keys(',
        'esp_efuse_write_field_',
        'esp_efuse_set_key_purpose(',
        'esp_efuse_set_read_protect(',
        'esp_efuse_set_write_protect(',
        'esp_efuse_destroy_block(',
    )
    for fragment in forbidden:
        assert fragment not in text, f'Station signer must not mutate eFuse state: {fragment}'


def test_efuse_signer_is_explicit_and_fail_closed():
    """The eFuse backend must require deliberate ESP32-C5 hardware configuration and validate key privacy."""
    signer = SIGNER.read_text(encoding='utf-8')
    kconfig = KCONFIG.read_text(encoding='utf-8')

    required_signer = (
        'ESP_EFUSE_KEY_PURPOSE_ECDSA_KEY_P256',
        'esp_efuse_get_key_dis_read',
        'PSA_KEY_LIFETIME_ESP_ECDSA_VOLATILE',
        'psa_sign_hash',
        'psa_export_public_key',
    )
    for fragment in required_signer:
        assert fragment in signer, f'eFuse signer lost fail-closed contract: {fragment}'

    required_config = (
        'depends on IDF_TARGET_ESP32C5 && MBEDTLS_HARDWARE_ECDSA_SIGN',
        'LASTRO_STATION_EFUSE_KEY_BLOCK_INDEX',
        'range 0 5',
    )
    for fragment in required_config:
        assert fragment in kconfig, f'eFuse configuration lost required constraint: {fragment}'

def test_board_entrypoint_wires_agent_transport_while_reader_protocol_stays_isolated():
    """Native host transport may be fixed; reader-specific bytes must remain behind the adapter boundary."""
    main = (ROOT / 'firmware/station/main/app_main.c').read_text(encoding='utf-8')
    cmake = (ROOT / 'firmware/station/main/CMakeLists.txt').read_text(encoding='utf-8')
    defaults = (ROOT / 'firmware/station/sdkconfig.defaults').read_text(encoding='utf-8')
    kconfig = KCONFIG.read_text(encoding='utf-8')
    required_main = (
        'usb_serial_jtag_driver_install',
        'usb_serial_jtag_read_bytes',
        'usb_serial_jtag_write_bytes',
        'lastro_runtime_init',
        'lastro_runtime_feed_agent',
        'lastro_rfid_poll',
        'lastro_rfid_take',
        'lastro_runtime_observe_rfid',
        'lastro_runtime_poll',
        'LASTRO_STATION_WAIT_RFID',
    )
    for fragment in required_main:
        assert fragment in main, f'board entrypoint lost Agent/runtime integration: {fragment}'
    assert 'lastro_rfid_feed' not in main
    assert 'esp_driver_usb_serial_jtag' in cmake
    assert 'CONFIG_ESP_CONSOLE_UART_DEFAULT=y' in defaults
    assert 'CONFIG_ESP_CONSOLE_SECONDARY_NONE=y' in defaults
    assert 'LASTRO_STATION_UART_RX_BUFFER' not in kconfig



def test_firmware_ci_compiles_efuse_signer_without_provisioning_commands():
    """CI must compile the hardware-backed signer configuration without claiming or mutating physical eFuse state."""
    workflow = (ROOT / '.github/workflows/firmware.yml').read_text(encoding='utf-8')
    assert '-DSDKCONFIG_DEFAULTS="sdkconfig.defaults;sdkconfig.efuse.defaults"' in workflow
    assert "CONFIG_MBEDTLS_HARDWARE_ECDSA_SIGN=y" in workflow
    assert "CONFIG_LASTRO_STATION_USE_EFUSE_KEY=y" in workflow
    assert "CONFIG_LASTRO_STATION_EFUSE_KEY_BLOCK_INDEX=0" in workflow
    for forbidden in ('espefuse burn', 'esp_efuse_write_key', 'esp_efuse_set_key_purpose'):
        assert forbidden not in workflow
