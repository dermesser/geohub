// Figure out client/secret from URL and/or UI field. Update UI field with URL value
// available.
function getClient() {
    var inputClient = document.getElementById('inputClient');
    var userClient = inputClient.value;
    var urlClient = urlParams.get('client');

    if (userClient.length == 0) {
        inputClient.value = urlClient;
        return urlClient;
    }
    return userClient;
}
function getSecret() {
    var inputSecret = document.getElementById('inputSecret');
    var userSecret = inputSecret.value;
    var urlSecret = urlParams.get('secret');

    if (userSecret.length == 0) {
        inputSecret.value = urlSecret;
        return urlSecret ? urlSecret : '';
    }
    return userSecret ? userSecret : '';
}


// Update URL from client/secret.
function updateURL(client, secret) {
    var url = window.location.toString();
    if (url.search('\\?') < 0) {
        url += '?';
    }

    if (url.search('secret=') > 0) {
        url = url.replace(/secret=[a-zA-Z0-9]*/, `secret=${secret}`);
    } else {
        url += `&secret=${secret}`;
    }

    if (url.search('client=') > 0) {
        url = url.replace(/client=[a-zA-Z0-9]*/, `client=${client}`);
    } else {
        url += `&client=${client}`;
    }

    window.history.pushState({}, "", url);
}
